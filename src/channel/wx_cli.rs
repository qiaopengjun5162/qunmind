use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use super::wechat_db::{self, RawWechatMessage};
use super::{Channel, IncomingMessage, MessageHandler, MsgType};
use crate::config::WxCliConfig;
use crate::error::{QunMindError, Result};

pub struct WxCliChannel {
    group_chat_id: String,
    poll_interval: tokio::time::Duration,
    seen: Mutex<HashSet<String>>,
    /// Last seen local_id for incremental polling.
    last_local_id: Mutex<i64>,
    // Legacy shell-out fallback — used when native memory scan is unavailable.
    bin: String,
    poll_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RenderedWxCliSendCommand {
    pub command: String,
}

impl WxCliChannel {
    pub fn new(config: &WxCliConfig) -> Self {
        Self {
            group_chat_id: config.group_chat_id.clone(),
            poll_interval: tokio::time::Duration::from_secs(config.poll_interval_secs),
            seen: Mutex::new(HashSet::new()),
            last_local_id: Mutex::new(0),
            bin: config.bin.clone(),
            poll_args: config.poll_args.clone(),
        }
    }

    fn has_legacy_config(&self) -> bool {
        !self.bin.is_empty() && !self.poll_args.is_empty() && self.bin != "wx" // default placeholder
    }

    /// Legacy shell-out poll — used when native memory scan is unavailable (CI/tests).
    async fn poll_legacy(&self) -> Result<Vec<RawWechatMessage>> {
        let output = tokio::process::Command::new(&self.bin)
            .args(&self.poll_args)
            .output()
            .await
            .map_err(|e| QunMindError::Channel(format!("wx-cli 拉取消息失败: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(QunMindError::Channel(format!(
                "wx-cli 拉取消息失败: {}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        let value: serde_json::Value = serde_json::from_str(&stdout)?;
        Ok(parse_messages(&value, &self.group_chat_id))
    }

    /// Poll WeChat once, returning only new messages since the last poll.
    ///
    /// On first call, extracts the SQLCipher key from WeChat process memory
    /// and opens the message database. Subsequent calls reuse the cached key.
    pub async fn poll_once(&self) -> Result<Vec<IncomingMessage>> {
        Ok(self
            .poll_messages()
            .await?
            .into_iter()
            .map(|m| into_incoming(m, &self.group_chat_id))
            .collect())
    }

    async fn poll_messages(&self) -> Result<Vec<RawWechatMessage>> {
        // Key resolution: live memory scan → cached keys on disk → legacy shell fallback.
        let db_keys = match wechat_db::extract_all_db_keys() {
            Ok(keys) => {
                // Persist so future runs survive binary rebuilds / SIP restarts.
                wechat_db::save_keys(&keys);
                keys
            }
            Err(scan_err) => {
                let cached = wechat_db::load_cached_keys();
                if !cached.is_empty() {
                    info!("内存扫描失败 ({scan_err}), 使用磁盘缓存密钥");
                    cached
                } else if self.has_legacy_config() {
                    debug!("内存扫描失败且无缓存密钥, 回退到 shell 路径");
                    return self.poll_legacy().await;
                } else {
                    return Err(QunMindError::Channel(format!(
                        "无法获取数据库密钥 (内存扫描: {scan_err})。\
                        请以 sudo 运行一次以建立密钥缓存，或用 `wx-cli keys-extract` 命令提取。"
                    )));
                }
            }
        };

        tracing::debug!("找到 {} 个候选密钥", db_keys.len());

        let db_paths = wechat_db::message_db_paths();
        if db_paths.is_empty() {
            return Err(QunMindError::Channel(
                "未找到微信消息数据库。请确认微信已登录。".to_string(),
            ));
        }

        let mut last_id = *self.last_local_id.lock().await;

        // Helper to decrypt + query with given keys
        let try_keys = |keys: &[String], last_id: &mut i64| -> Vec<RawWechatMessage> {
            let mut msgs = Vec::new();
            for db_path in &db_paths {
                let plain_path = match wechat_db::decrypt_db(db_path, keys) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("跳过数据库 (密钥未匹配): {e}");
                        continue;
                    }
                };
                let conn = match wechat_db::open_decrypted_db(&plain_path) {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(path = %db_path.display(), "无法打开解密数据库: {e}");
                        continue;
                    }
                };
                match wechat_db::query_new_messages(&conn, *last_id, 200) {
                    Ok(messages) => {
                        if !messages.is_empty() {
                            if let Some(max_msg) = messages.iter().max_by_key(|m| m.local_id) {
                                let new_max = max_msg.local_id;
                                if new_max > *last_id {
                                    *last_id = new_max;
                                }
                            }
                            msgs.extend(messages);
                        }
                    }
                    Err(e) => warn!(path = %db_path.display(), "查询消息失败: {e}"),
                }
                drop(conn);
                let _ = std::fs::remove_file(&plain_path);
                if let Some(parent) = plain_path.parent() {
                    let _ = std::fs::remove_dir(parent);
                }
            }
            msgs
        };

        let mut all_messages = try_keys(&db_keys, &mut last_id);

        all_messages.sort_by_key(|m| m.local_id);
        *self.last_local_id.lock().await = last_id;
        Ok(all_messages)
    }

    pub fn rendered_send_command(
        &self,
        chat_id: &str,
        text: &str,
    ) -> Result<RenderedWxCliSendCommand> {
        let applescript = render_send_applescript(chat_id, text);
        Ok(RenderedWxCliSendCommand {
            command: format!("osascript -e '{applescript}'"),
        })
    }
}

/// Convert a raw WeChat database row into the normalized incoming message.
fn into_incoming(msg: RawWechatMessage, fallback_chat_id: &str) -> IncomingMessage {
    let chat_id = if msg.talker.ends_with("@chatroom") {
        msg.talker.clone()
    } else if !fallback_chat_id.is_empty() {
        fallback_chat_id.to_string()
    } else {
        msg.talker.clone()
    };

    let is_group =
        chat_id.ends_with("@chatroom") || chat_id.starts_with("@@") || !fallback_chat_id.is_empty();

    let message_id = if !msg.original_id.is_empty() {
        msg.original_id.clone()
    } else {
        format!("wx-{}", msg.local_id)
    };
    IncomingMessage {
        message_id,
        from: String::new(),
        chat_id,
        is_group,
        text: Some(parse_message_content(&msg.content, msg.msg_type)),
        msg_type: wechat_msg_type(msg.msg_type),
    }
}

/// WeChat message types (partial, covering common cases).
fn wechat_msg_type(msg_type: i64) -> MsgType {
    match msg_type {
        1 => MsgType::Text,
        3 => MsgType::Image,
        34 => MsgType::Voice,
        43 => MsgType::Unknown, // video (no dedicated variant)
        47 => MsgType::Image,   // emoji / sticker
        49 => MsgType::File,    // appmsg (link, file, quote, etc.)
        _ => MsgType::Unknown,
    }
}

/// Extract plain text from WeChat message content.
///
/// WeChat stores rich messages as XML; extract the text portion.
fn parse_message_content(content: &str, msg_type: i64) -> String {
    match msg_type {
        1 => content.to_string(), // plain text
        49 => {
            // AppMsg XML — try to extract title/description.
            if let Some(title) = extract_xml_field(content, "title") {
                return title;
            }
            content.to_string()
        }
        _ => {
            if content.trim().is_empty() || content.starts_with('<') {
                String::new()
            } else {
                content.to_string()
            }
        }
    }
}

fn extract_xml_field(xml: &str, field: &str) -> Option<String> {
    let open = format!("<{field}>");
    let close = format!("</{field}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    let value = xml[start..start + end].trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

// ---------------------------------------------------------------------------
// AppleScript sender — uses macOS built-in osascript (not a third-party tool)
// ---------------------------------------------------------------------------

fn render_send_applescript(chat_name: &str, text: &str) -> String {
    // Escape single quotes for AppleScript string literals.
    let escaped_text = text.replace('\\', "\\\\").replace('\'', "'\"'\"'");
    let escaped_name = chat_name.replace('\\', "\\\\").replace('\'', "'\"'\"'");

    format!(
        r#"tell application "System Events"
    tell process "WeChat"
        set frontmost to true
        delay 0.3
        keystroke "f" using command down
        delay 0.5
        keystroke "{escaped_name}"
        delay 0.8
        key code 36
        delay 0.5
        keystroke "{escaped_text}"
        delay 0.3
        key code 36
    end tell
end tell"#
    )
}

// ---------------------------------------------------------------------------
// Channel trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl Channel for WxCliChannel {
    fn name(&self) -> &str {
        "wx_cli"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<()> {
        info!("wx-cli 原生通道启动（内存扫描 + SQLCipher 直读）");

        loop {
            match self.poll_messages().await {
                Ok(messages) => {
                    for msg in messages {
                        let msg_id = format!("wx-{}", msg.local_id);
                        let mut seen = self.seen.lock().await;
                        if !seen.insert(msg_id.clone()) {
                            continue;
                        }
                        drop(seen);

                        let incoming = into_incoming(msg, &self.group_chat_id);
                        let handler = Arc::clone(&handler);
                        tokio::spawn(async move {
                            if let Err(e) = handler.on_message(incoming).await {
                                error!("处理 wx-cli 消息失败: {}", e);
                            }
                        });
                    }
                }
                Err(e) => warn!("wx-cli 拉取消息失败: {}", e),
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn send_text(&self, chat_id: &str, text: &str) -> Result<()> {
        let script = render_send_applescript(chat_id, text);
        debug!(chat_id, text_len = text.len(), "osascript 发送微信消息");

        let output = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .await
            .map_err(|e| QunMindError::Channel(format!("osascript 执行失败: {e}")))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(QunMindError::Channel(format!(
                "微信发送失败: {}",
                stderr.trim()
            )))
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Legacy parse_wx_cli_messages_from_str — kept for captured JSON replay
// ---------------------------------------------------------------------------

pub fn parse_wx_cli_messages_from_str(
    raw: &str,
    fallback_chat_id: &str,
) -> Result<Vec<IncomingMessage>> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let value: Value = serde_json::from_str(raw)?;
    Ok(parse_messages(&value, fallback_chat_id)
        .into_iter()
        .map(Into::into)
        .collect())
}

// Re-use existing parsing for captured JSON replay compatibility.
fn parse_messages(value: &Value, fallback_chat_id: &str) -> Vec<RawWechatMessage> {
    candidate_arrays(value)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|item| parse_message_from_json(item, fallback_chat_id))
        .collect()
}

fn candidate_arrays(value: &Value) -> Vec<&Vec<Value>> {
    let mut arrays = Vec::new();
    collect_candidate_arrays(value, &mut arrays);
    arrays
}

fn collect_candidate_arrays<'a>(value: &'a Value, arrays: &mut Vec<&'a Vec<Value>>) {
    match value {
        Value::Array(items) => arrays.push(items),
        Value::Object(map) => {
            for key in ["messages", "msg_list", "results", "items", "data"] {
                if let Some(child) = map.get(key) {
                    collect_candidate_arrays(child, arrays);
                }
            }
        }
        _ => {}
    }
}

fn parse_message_from_json(value: &Value, fallback_chat_id: &str) -> Option<RawWechatMessage> {
    let text = first_string(
        value,
        &[
            "content",
            "text",
            "message",
            "body",
            "msg",
            "msg_content",
            "plain_text",
        ],
    )?;

    let chat_id = match first_string(
        value,
        &[
            "chat_id",
            "room_id",
            "roomid",
            "talker",
            "session",
            "chat",
            "conversation_id",
        ],
    ) {
        Some(c) => c,
        None => fallback_chat_id.to_string(),
    };

    if chat_id.is_empty() {
        return None;
    }

    let id = match first_string(
        value,
        &[
            "id",
            "msg_id",
            "message_id",
            "local_id",
            "server_id",
            "client_msg_id",
        ],
    ) {
        Some(i) => i,
        None => format!("{chat_id}:{}", stable_text_id(&text)),
    };

    // Use numeric local_id from DB, or stable hash for string IDs from captured files.
    let local_id: i64 = id.parse().unwrap_or_else(|_| stable_text_id(&id) as i64);

    Some(RawWechatMessage {
        local_id,
        svr_id: 0,
        create_time: 0,
        talker: chat_id,
        content: text,
        msg_type: 1,
        is_sender: false,
        original_id: id,
    })
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    let Value::Object(map) = value else {
        return None;
    };

    for key in keys {
        match map.get(*key) {
            Some(Value::String(v)) if !v.is_empty() => return Some(v.clone()),
            Some(Value::Number(v)) => return Some(v.to_string()),
            Some(Value::Object(_)) => {
                if let Some(child) = first_string(map.get(*key)?, keys) {
                    return Some(child);
                }
            }
            _ => {}
        }
    }

    None
}

fn stable_text_id(text: &str) -> u64 {
    text.bytes().fold(14_695_981_039_346_656_037, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(1_099_511_628_211)
    })
}

impl From<RawWechatMessage> for IncomingMessage {
    fn from(msg: RawWechatMessage) -> Self {
        // Captured JSON files always set original_id; DB messages leave it empty.
        let message_id = if !msg.original_id.is_empty() {
            msg.original_id.clone()
        } else {
            format!("wx-{}", msg.local_id)
        };
        IncomingMessage {
            message_id,
            from: String::new(),
            chat_id: msg.talker.clone(),
            is_group: msg.talker.ends_with("@chatroom"),
            text: Some(parse_message_content(&msg.content, msg.msg_type)),
            msg_type: wechat_msg_type(msg.msg_type),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_raw_messages(raw: &str, fallback_chat_id: &str) -> Vec<IncomingMessage> {
        match parse_wx_cli_messages_from_str(raw, fallback_chat_id) {
            Ok(messages) => messages,
            Err(err) => panic!("messages: {err}"),
        }
    }

    #[test]
    fn parses_wrapped_wx_cli_messages() {
        let value = json!({
            "results": [
                {
                    "local_id": 42,
                    "chat": "rust群",
                    "sender": "alice",
                    "content": "@bot 总结一下",
                    "chat_type": "group"
                }
            ],
            "meta": { "status": "ok" }
        });

        let messages = parse_messages(&value, "");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].local_id, 42);
        assert_eq!(messages[0].talker, "rust群");
        assert_eq!(messages[0].content, "@bot 总结一下");
    }

    #[test]
    fn parses_wx_cli_messages_from_raw_json() {
        let messages = parse_raw_messages(
            r#"
            {
                "messages": [
                    {
                        "id": "100",
                        "talker": "room@chatroom",
                        "sender": "alice",
                        "content": "@bot raw hello"
                    }
                ]
            }
            "#,
            "",
        );

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, "100");
        assert_eq!(messages[0].chat_id, "room@chatroom");
        assert_eq!(messages[0].text.as_deref(), Some("@bot raw hello"));
        assert!(messages[0].is_group);
    }

    #[test]
    fn parses_empty_raw_json_as_no_messages() {
        let messages = parse_raw_messages("  ", "");
        assert!(messages.is_empty());
    }

    #[test]
    fn skips_messages_without_chat_id() {
        let value = json!([{ "content": "hello" }]);
        let messages = parse_messages(&value, "");
        assert!(messages.is_empty());
    }

    #[test]
    fn parse_message_content_handles_text() {
        assert_eq!(parse_message_content("hello", 1), "hello");
    }

    #[test]
    fn parse_message_content_extracts_appmsg_title() {
        let xml = r#"<msg><appmsg><title>Rust Blog</title><des>desc</des></appmsg></msg>"#;
        assert_eq!(parse_message_content(xml, 49), "Rust Blog");
    }

    #[test]
    fn extract_xml_field_returns_none_for_missing() {
        assert_eq!(extract_xml_field("<a>b</a>", "c"), None);
    }

    #[test]
    fn wechat_msg_type_maps_common_types() {
        assert_eq!(wechat_msg_type(1), MsgType::Text);
        assert_eq!(wechat_msg_type(3), MsgType::Image);
        assert_eq!(wechat_msg_type(34), MsgType::Voice);
        assert_eq!(wechat_msg_type(43), MsgType::Unknown);
        assert_eq!(wechat_msg_type(49), MsgType::File);
        assert_eq!(wechat_msg_type(999), MsgType::Unknown);
    }

    #[test]
    fn into_incoming_uses_fallback_chat_id() {
        let raw = RawWechatMessage {
            local_id: 1,
            svr_id: 0,
            create_time: 0,
            talker: "wxid_alice".to_string(),
            content: "hi".to_string(),
            msg_type: 1,
            is_sender: false,
            original_id: String::new(),
        };

        let msg = into_incoming(raw, "fallback@chatroom");
        assert_eq!(msg.chat_id, "fallback@chatroom");
        assert!(msg.is_group);
    }

    #[test]
    fn rendered_send_command_includes_osascript() {
        let channel = WxCliChannel::new(&WxCliConfig::default());
        let cmd = channel.rendered_send_command("测试群", "hello").unwrap();
        assert!(cmd.command.starts_with("osascript"));
        assert!(cmd.command.contains("测试群"));
        assert!(cmd.command.contains("hello"));
    }
}
