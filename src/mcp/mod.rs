mod tools;

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::config::Config;
use crate::mcp::tools::{Tool, call_tool, list_tools};

/// Run the MCP JSON-RPC 2.0 server on stdio.
///
/// Reads newline-delimited JSON requests from stdin, dispatches to the
/// appropriate handler, and writes JSON responses to stdout. Stderr is
/// reserved for `tracing` log output so it stays out of the protocol stream.
pub async fn run(config_path: PathBuf) -> anyhow::Result<()> {
    let config = Config::load(&config_path)?;
    let config = Arc::new(config);

    let stdin = std::io::stdin();
    let stdout = Arc::new(Mutex::new(std::io::stdout()));
    let reader = std::io::BufReader::new(stdin.lock());

    let mut initialized = false;

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                error!("stdin read error: {e}");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                error!("JSON parse error: {e}");
                continue;
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        debug!(%method, id = ?id, "MCP request");

        let response = match method {
            "initialize" => {
                let result = handle_initialize(&request);
                initialized = true;
                build_response(id, Some(result), None)
            }
            "notifications/initialized" => {
                // Acknowledged — no response needed per MCP spec.
                debug!("MCP client initialized");
                continue;
            }
            "tools/list" if initialized => {
                let result = handle_tools_list();
                build_response(id, Some(result), None)
            }
            "tools/call" if initialized => handle_tools_call(&config, &config_path, &request).await,
            _ if !initialized => {
                build_response(id, None, Some(jsonrpc_error(-32002, "Not initialized")))
            }
            _ => build_response(
                id,
                None,
                Some(jsonrpc_error(
                    -32601,
                    &format!("Method not found: {method}"),
                )),
            ),
        };

        let mut out = stdout.lock().await;
        if let Ok(json) = serde_json::to_string(&response) {
            let _ = writeln!(out, "{json}");
            let _ = out.flush();
        }
    }

    info!("MCP server stopped");
    Ok(())
}

fn handle_initialize(request: &Value) -> Value {
    let client_info = request
        .pointer("/params/clientInfo")
        .cloned()
        .unwrap_or(Value::Null);
    info!(?client_info, "MCP client connected");

    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn handle_tools_list() -> Value {
    let tools: Vec<Tool> = list_tools();
    serde_json::json!({
        "tools": tools
    })
}

async fn handle_tools_call(
    config: &Config,
    config_path: &std::path::Path,
    request: &Value,
) -> Value {
    let id = request.get("id").cloned();
    let params = match request.get("params") {
        Some(p) => p,
        None => {
            return build_response(id, None, Some(jsonrpc_error(-32602, "Missing params")));
        }
    };

    let tool_name = match params.get("name").and_then(|n| n.as_str()) {
        Some(n) => n,
        None => {
            return build_response(id, None, Some(jsonrpc_error(-32602, "Missing tool name")));
        }
    };

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(Value::Object(serde_json::Map::new()));

    match call_tool(config, config_path, tool_name, &arguments).await {
        Ok(content) => build_response(
            id,
            Some(serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": content
                }]
            })),
            None,
        ),
        Err(e) => {
            let error_text = format!("Tool '{tool_name}' failed: {e}");
            build_response(
                id,
                Some(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::json!({
                            "ok": false,
                            "error": error_text
                        }).to_string()
                    }],
                    "isError": true
                })),
                None,
            )
        }
    }
}

fn build_response(id: Option<Value>, result: Option<Value>, error: Option<Value>) -> Value {
    let mut response = serde_json::json!({
        "jsonrpc": "2.0"
    });

    if let Some(id) = id {
        response["id"] = id;
    }

    if let Some(result) = result {
        response["result"] = result;
    }

    if let Some(error) = error {
        response["error"] = error;
    }

    response
}

fn jsonrpc_error(code: i32, message: &str) -> Value {
    serde_json::json!({
        "code": code,
        "message": message
    })
}
