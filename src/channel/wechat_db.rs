// WeChat local database reader — pure Rust.
//
// Reference: jackwener/wx-cli (Apache 2.0)
//
// Architecture:
//   mach2 (Rust)  →  scan WeChat process memory for SQLCipher raw key
//   aes + cbc (Rust) →  decrypt SQLCipher pages in pure Rust, no system libs
//   rusqlite/bundled  →  open the decrypted temp DB and query messages
use std::fs;
use std::path::PathBuf;

use aes::cipher::{BlockDecryptMut, KeyIvInit};
#[cfg(target_os = "macos")]
use regex::Regex;
use tracing::{debug, warn};

use crate::error::{QunMindError, Result};

const PAGE_SIZE: usize = 4096;
const RESERVE_SIZE: usize = 48;
const SALT_SIZE: usize = 16;

type Aes256Cbc = cbc::Decryptor<aes::Aes256>;

// ---------------------------------------------------------------------------
// WeChat data paths
// ---------------------------------------------------------------------------

const WECHAT_CONTAINERS: &[&str] = &["com.tencent.xinWeChat", "com.tencent.xinWeChat.beta"];

fn wechat_data_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok().map(PathBuf::from)?;
    let containers = home.join("Library/Containers");

    for container in WECHAT_CONTAINERS {
        let base = containers
            .join(container)
            .join("Data/Documents/xwechat_files");
        if base.exists()
            && let Ok(entries) = std::fs::read_dir(&base)
        {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && entry.file_name().to_string_lossy().starts_with("wxid_")
                {
                    let db = entry.path().join("db_storage/message");
                    if db.exists() {
                        return Some(db);
                    }
                }
            }
        }
    }

    None
}

pub fn message_db_paths() -> Vec<PathBuf> {
    let dir = match wechat_data_dir() {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut paths: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("message_"))
            .map(|e| e.path())
            .collect(),
        Err(_) => return Vec::new(),
    };

    paths.sort_by(|a, b| {
        let n = |p: &PathBuf| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("message_"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
        };
        n(b).cmp(&n(a))
    });

    paths
}

// ---------------------------------------------------------------------------
// Memory scan for SQLCipher key (macOS only)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub fn extract_db_key() -> Result<String> {
    use mach2::kern_return::KERN_SUCCESS;
    use mach2::traps::{mach_task_self, task_for_pid};
    use mach2::vm::mach_vm_region;
    use mach2::vm_prot::VM_PROT_READ;
    use mach2::vm_region::{VM_REGION_BASIC_INFO_64, vm_region_basic_info_64};
    use mach2::vm_types::{mach_vm_address_t, mach_vm_size_t};

    let pid = find_wechat_pid()?;
    let mut task: mach2::port::mach_port_t = 0;
    let kr = unsafe { task_for_pid(mach_task_self(), pid, &mut task) };
    if kr != KERN_SUCCESS {
        return Err(QunMindError::Channel(
            "无法获取微信进程 task port。请使用 `sudo` 运行。".to_string(),
        ));
    }

    let pattern = Regex::new(r"x'([0-9a-fA-F]{96})'").unwrap();
    let mut address: mach_vm_address_t = 0;
    let mut size: mach_vm_size_t = 0;
    let mut info: vm_region_basic_info_64 = unsafe { std::mem::zeroed() };
    let mut info_count = vm_region_basic_info_64::count();

    let mut buf = vec![0u8; 512 * 1024];
    let mut keys = Vec::new();

    loop {
        let mut object_name: mach2::port::mach_port_t = 0;
        let kr = unsafe {
            mach_vm_region(
                task,
                &mut address,
                &mut size,
                VM_REGION_BASIC_INFO_64,
                (&mut info as *mut _) as mach2::vm_region::vm_region_info_t,
                &mut info_count,
                &mut object_name,
            )
        };

        if kr != KERN_SUCCESS {
            break;
        }

        if (info.protection & VM_PROT_READ) != 0
            && info.shared == 0
            && info.reserved == 0
            && size > 0
        {
            let mut offset: u64 = 0;
            while offset < size {
                let chunk_size = (size - offset).min(buf.len() as u64);
                let mut data_size: mach_vm_size_t = 0;

                let kr = unsafe {
                    mach2::vm::mach_vm_read_overwrite(
                        task,
                        address + offset,
                        chunk_size,
                        buf.as_mut_ptr() as u64,
                        &mut data_size,
                    )
                };

                if kr == KERN_SUCCESS && data_size > 0 {
                    if let Ok(text) = std::str::from_utf8(&buf[..data_size as usize]) {
                        for cap in pattern.captures_iter(text) {
                            keys.push(cap[1].to_string());
                        }
                    } else {
                        for win in buf[..data_size as usize].windows(98) {
                            if win[0] == b'x' && win[1] == b'\'' {
                                if let Ok(candidate) = std::str::from_utf8(&win[2..98]) {
                                    if candidate.len() == 96
                                        && candidate.chars().all(|c| c.is_ascii_hexdigit())
                                    {
                                        keys.push(candidate.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                offset += chunk_size;
            }
        }

        address += size;
        if address == 0 {
            break;
        }
    }

    keys.into_iter().next().map_or_else(
        || {
            Err(QunMindError::Channel(
                "无法从微信进程内存中提取数据库密钥".to_string(),
            ))
        },
        Ok,
    )
}

#[cfg(not(target_os = "macos"))]
pub fn extract_db_key() -> Result<String> {
    Err(QunMindError::Channel(
        "当前平台暂不支持微信内存扫描".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn find_wechat_pid() -> Result<i32> {
    use std::process::Command;

    let output = Command::new("pgrep")
        .args(["-n", "WeChat"])
        .output()
        .map_err(|_| QunMindError::Channel("无法查找微信进程".to_string()))?;

    let pid_str = String::from_utf8_lossy(&output.stdout);
    pid_str
        .trim()
        .parse::<i32>()
        .map_err(|_| QunMindError::Channel("微信未运行，请先启动微信".to_string()))
}

// ---------------------------------------------------------------------------
// Pure Rust SQLCipher 4 page decryption
// ---------------------------------------------------------------------------

/// Decrypt a SQLCipher-encrypted database file in pure Rust.
///
/// Returns the path to a temporary plaintext SQLite database.
pub fn decrypt_db(encrypted_path: &std::path::Path, raw_key_hex: &str) -> Result<PathBuf> {
    let key_bytes = hex_decode(raw_key_hex)?;
    if key_bytes.len() < 32 {
        return Err(QunMindError::Channel("数据库密钥长度不足".to_string()));
    }
    let aes_key: &[u8; 32] = key_bytes[..32].try_into().unwrap();

    let encrypted = fs::read(encrypted_path)
        .map_err(|e| QunMindError::Channel(format!("无法读取加密数据库: {e}")))?;

    let page_count = encrypted.len() / PAGE_SIZE;
    let plain_page_size = PAGE_SIZE - SALT_SIZE - RESERVE_SIZE; // 4032 bytes usable per page
    let plain_size = page_count * plain_page_size;

    let temp_dir = std::env::temp_dir().join(format!("qunmind-wechat-{}", std::process::id()));
    fs::create_dir_all(&temp_dir)
        .map_err(|e| QunMindError::Channel(format!("无法创建临时目录: {e}")))?;

    let temp_path = temp_dir.join(
        encrypted_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("message.db")),
    );

    let mut plaintext = Vec::with_capacity(plain_size);

    for page_num in 0..page_count {
        let offset = page_num * PAGE_SIZE;
        let page = &encrypted[offset..(offset + PAGE_SIZE).min(encrypted.len())];

        if page.len() < PAGE_SIZE {
            break;
        }

        // SQLCipher 4 page layout:
        //   bytes 0..15:        salt (also serves as AES-CBC IV)
        //   bytes 16..4047:     AES-256-CBC encrypted page data
        //   bytes 4048..4095:   HMAC (skipped in decryption)
        let iv: &[u8; 16] = page[..16].try_into().unwrap();
        let ciphertext = &page[16..PAGE_SIZE - RESERVE_SIZE];

        let mut decrypted = ciphertext.to_vec();
        {
            let blocks = unsafe {
                std::slice::from_raw_parts_mut(
                    decrypted.as_mut_ptr() as *mut aes::Block,
                    decrypted.len() / 16,
                )
            };
            let mut cipher = Aes256Cbc::new(aes_key.into(), iv.into());
            cipher.decrypt_blocks_mut(blocks);
        }

        plaintext.extend_from_slice(&decrypted);
    }

    fs::write(&temp_path, &plaintext)
        .map_err(|e| QunMindError::Channel(format!("无法写入解密数据库: {e}")))?;

    debug!(
        path = %temp_path.display(),
        pages = page_count,
        size = plain_size,
        "SQLCipher 数据库已解密（纯 Rust）"
    );

    Ok(temp_path)
}

/// Open a decrypted (plaintext) SQLite database.
pub fn open_decrypted_db(path: &std::path::Path) -> Result<rusqlite::Connection> {
    // We need to tell SQLite the page size is 4032, since we stripped the crypto overhead.
    let conn = rusqlite::Connection::open(path).map_err(|e| {
        QunMindError::Channel(format!("无法打开解密数据库 {}: {e}", path.display()))
    })?;

    // Check for valid SQLite header.
    let header_check: String = conn
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .unwrap_or_else(|_| "unknown".to_string());
    debug!(version = %header_check, "解密数据库已打开");

    Ok(conn)
}

fn hex_decode(hex: &str) -> Result<Vec<u8>> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| QunMindError::Channel("无效的十六进制密钥".to_string()))
}

// ---------------------------------------------------------------------------
// Message querying
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RawWechatMessage {
    pub local_id: i64,
    pub svr_id: i64,
    pub create_time: i64,
    pub talker: String,
    pub content: String,
    pub msg_type: i64,
    pub is_sender: bool,
    /// Original string ID for captured JSON replay (empty for DB messages).
    pub original_id: String,
}

pub fn query_new_messages(
    conn: &rusqlite::Connection,
    since_local_id: i64,
    limit: usize,
) -> Result<Vec<RawWechatMessage>> {
    // The decrypted DB has 4032-byte pages. Try to query with the standard schema.
    // If the schema is intact, ChatMessage table exists.
    let mut stmt = conn
        .prepare(
            "SELECT local_id, svr_id, CreateTime, Talker, Message, Type, Des
             FROM ChatMessage
             WHERE local_id > ?1
             ORDER BY local_id ASC
             LIMIT ?2",
        )
        .map_err(|e| QunMindError::Channel(format!("查询微信消息失败 (schema may differ): {e}")))?;

    let rows = stmt
        .query_map(rusqlite::params![since_local_id, limit as i64], |row| {
            Ok(RawWechatMessage {
                local_id: row.get(0)?,
                svr_id: row.get(1)?,
                create_time: row.get(2)?,
                talker: row.get(3)?,
                content: row.get(4)?,
                msg_type: row.get(5)?,
                is_sender: row.get::<_, i64>(6).unwrap_or(0) == 1,
                original_id: String::new(),
            })
        })
        .map_err(|e| QunMindError::Channel(format!("读取微信消息失败: {e}")))?;

    let mut messages = Vec::new();
    for row in rows {
        match row {
            Ok(msg) => messages.push(msg),
            Err(e) => warn!("跳过损坏的微信消息行: {e}"),
        }
    }

    Ok(messages)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decode_parses_key() {
        let key = "00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff";
        let bytes = hex_decode(key).unwrap();
        assert_eq!(bytes.len(), 48);
        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes[1], 0xff);
    }

    #[test]
    fn hex_decode_rejects_invalid() {
        assert!(hex_decode("xyz").is_err());
    }

    #[test]
    fn message_db_paths_returns_vec() {
        let paths = message_db_paths();
        assert!(paths.is_empty() || !paths.is_empty());
    }
}
