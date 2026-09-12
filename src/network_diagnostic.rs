use crate::reporting::ReportStatusTarget;
use serde_json::{Value, json};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const DEFAULT_LOCAL_PROXY: &str = "127.0.0.1:7890";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkDiagnosticOptions {
    pub proxy_url: String,
    pub mihomo_controller: String,
    pub mihomo_secret_set: bool,
}

impl Default for NetworkDiagnosticOptions {
    fn default() -> Self {
        Self {
            proxy_url: format!("http://{DEFAULT_LOCAL_PROXY}"),
            mihomo_controller: String::new(),
            mihomo_secret_set: false,
        }
    }
}

impl NetworkDiagnosticOptions {
    pub fn from_env() -> Self {
        Self {
            proxy_url: std::env::var("QUNMIND_PUBLISH_PROXY")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("http://{DEFAULT_LOCAL_PROXY}")),
            mihomo_controller: std::env::var("MIHOMO_CONTROLLER")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_default(),
            mihomo_secret_set: std::env::var("MIHOMO_SECRET")
                .ok()
                .is_some_and(|value| !value.trim().is_empty()),
        }
    }
}

pub fn report_network_status_json(
    report_name: &str,
    target: &ReportStatusTarget,
    options: &NetworkDiagnosticOptions,
) -> Value {
    let local_proxy = local_proxy_json(&options.proxy_url);
    let local_proxy_reachable = local_proxy["reachable"].as_bool().unwrap_or(false);
    let mihomo_configured = !options.mihomo_controller.trim().is_empty();

    json!({
        "ok": target.output != "wechat" || local_proxy_reachable || mihomo_configured,
        "report_name": report_name,
        "target": {
            "chat_id": target.chat_id,
            "output": target.output,
            "wechat_bin_configured": !target.wechat_bin.trim().is_empty(),
            "wechat_articles_dir_configured": !target.wechat_articles_dir.trim().is_empty(),
        },
        "proxy_env": proxy_env_json(),
        "local_proxy": local_proxy,
        "mihomo": {
            "controller_configured": mihomo_configured,
            "controller": redact_controller(&options.mihomo_controller),
            "secret_configured": options.mihomo_secret_set,
            "note": "read-only diagnostic; QunMind does not mutate Mihomo or Clash profiles",
        },
        "wechat_openapi": {
            "host": "api.weixin.qq.com",
            "common_invalid_ip_error": "errcode=40164 invalid ip",
            "recommendation": if target.output == "wechat" {
                "verify the WeChat-reported exit IP and add a stable fixed-node IP to the allowlist"
            } else {
                "not a wechat publish target"
            },
        },
        "next_steps": next_steps(target, local_proxy_reachable, mihomo_configured),
    })
}

fn proxy_env_json() -> Value {
    json!({
        "HTTP_PROXY": env_state("HTTP_PROXY"),
        "HTTPS_PROXY": env_state("HTTPS_PROXY"),
        "ALL_PROXY": env_state("ALL_PROXY"),
        "http_proxy": env_state("http_proxy"),
        "https_proxy": env_state("https_proxy"),
        "all_proxy": env_state("all_proxy"),
        "QUNMIND_PUBLISH_PROXY": env_state("QUNMIND_PUBLISH_PROXY"),
    })
}

fn env_state(name: &str) -> &'static str {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => "set",
        _ => "unset",
    }
}

fn local_proxy_json(proxy_url: &str) -> Value {
    let address =
        proxy_socket_address(proxy_url).unwrap_or_else(|| DEFAULT_LOCAL_PROXY.to_string());
    let reachable = tcp_reachable(&address);

    json!({
        "url": redact_proxy_url(proxy_url),
        "address": address,
        "reachable": reachable,
    })
}

fn proxy_socket_address(proxy_url: &str) -> Option<String> {
    let without_scheme = proxy_url
        .strip_prefix("http://")
        .or_else(|| proxy_url.strip_prefix("https://"))
        .or_else(|| proxy_url.strip_prefix("socks5://"))
        .or_else(|| proxy_url.strip_prefix("socks5h://"))
        .unwrap_or(proxy_url);
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    if host_port.contains('@') {
        return host_port.rsplit('@').next().map(ToString::to_string);
    }
    if host_port.contains(':') {
        Some(host_port.to_string())
    } else {
        None
    }
}

fn tcp_reachable(address: &str) -> bool {
    address
        .to_socket_addrs()
        .ok()
        .and_then(|mut addresses| addresses.next())
        .is_some_and(|address| {
            TcpStream::connect_timeout(&address, Duration::from_millis(300)).is_ok()
        })
}

fn redact_proxy_url(proxy_url: &str) -> String {
    if let Some((scheme, rest)) = proxy_url.split_once("://")
        && let Some(host) = rest.rsplit('@').next()
    {
        return format!("{scheme}://{host}");
    }
    proxy_url.to_string()
}

fn redact_controller(controller: &str) -> String {
    if controller.trim().is_empty() {
        return String::new();
    }
    redact_proxy_url(controller)
}

fn next_steps(
    target: &ReportStatusTarget,
    local_proxy_reachable: bool,
    mihomo_configured: bool,
) -> Vec<&'static str> {
    if target.output != "wechat" {
        return vec!["report_network_status_is_only_needed_for_wechat_publish_targets"];
    }

    let mut steps = Vec::new();
    if !local_proxy_reachable {
        steps.push("start_or_fix_local_proxy_then_rerun_report_network_status");
    }
    if !mihomo_configured {
        steps.push("optionally_set_mihomo_controller_for_route_diagnostics");
    }
    steps.push("verify_wechat_openapi_exit_ip_before_publish");
    steps.push("prefer_fixed_node_exit_over_cloudflare_anycast_for_wechat_allowlist");
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(output: &str) -> ReportStatusTarget {
        ReportStatusTarget {
            chat_id: "group-1".to_string(),
            output: output.to_string(),
            wechat_bin: "moonpub".to_string(),
            wechat_articles_dir: "/tmp/articles".to_string(),
        }
    }

    #[test]
    fn proxy_socket_address_accepts_common_proxy_urls() {
        assert_eq!(
            proxy_socket_address("http://127.0.0.1:7890").as_deref(),
            Some("127.0.0.1:7890")
        );
        assert_eq!(
            proxy_socket_address("socks5h://user:pass@127.0.0.1:7891").as_deref(),
            Some("127.0.0.1:7891")
        );
    }

    #[test]
    fn report_network_status_redacts_proxy_credentials() {
        let report = report_network_status_json(
            "微信公众号日报",
            &target("wechat"),
            &NetworkDiagnosticOptions {
                proxy_url: "http://user:pass@127.0.0.1:7890".to_string(),
                mihomo_controller: "http://secret@127.0.0.1:9090".to_string(),
                mihomo_secret_set: true,
            },
        );

        assert_eq!(report["local_proxy"]["url"], "http://127.0.0.1:7890");
        assert_eq!(report["mihomo"]["controller"], "http://127.0.0.1:9090");
        assert_eq!(report["mihomo"]["secret_configured"], true);
    }

    #[test]
    fn report_network_status_marks_non_wechat_as_not_relevant() {
        let report = report_network_status_json(
            "普通日报",
            &target("channel"),
            &NetworkDiagnosticOptions::default(),
        );

        assert_eq!(
            report["next_steps"],
            json!(["report_network_status_is_only_needed_for_wechat_publish_targets"])
        );
    }
}
