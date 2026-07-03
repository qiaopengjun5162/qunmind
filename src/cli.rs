use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "qunmind", about = "微信群 AI 群智中枢")]
pub struct Args {
    /// Path to the local configuration file.
    #[arg(short, long, default_value = "config.toml", global = true)]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Diagnose wx-cli receive/send commands without starting the bot loop.
    #[command(name = "wx-cli")]
    WxCli {
        #[command(subcommand)]
        command: WxCliCommand,
    },
    /// Start an MCP (Model Context Protocol) server on stdio so AI agents can drive diagnostics.
    #[command(name = "mcp")]
    Mcp,
    /// 生成日报 markdown 并输出到文件（手动测试用）
    #[command(name = "daily-report")]
    DailyReport {
        /// 输出文件路径
        #[arg(long)]
        output: PathBuf,
        /// 已配置日报目标名称；用于复用该目标的 daily_quote / output 配置
        #[arg(long, default_value = "")]
        report_name: String,
        /// 回溯小时数
        #[arg(long, default_value_t = 24)]
        hours: i64,
        /// 生成 markdown 后，按目标配置继续执行发布
        #[arg(long)]
        publish: bool,
    },
    /// 查看最近的日报发布回执
    #[command(name = "publish-history")]
    PublishHistory {
        /// 日报目标名称；为空时使用 legacy daily_report_chat_id 兼容名称
        #[arg(long, default_value = "")]
        report_name: String,
        /// 最多返回多少条记录
        #[arg(long, default_value_t = 5)]
        limit: i64,
    },
    /// 查看日报发布就绪状态与最近发布记录
    #[command(name = "report-status")]
    ReportStatus {
        /// 日报目标名称；为空时使用 legacy daily_report_chat_id 兼容名称
        #[arg(long, default_value = "")]
        report_name: String,
        /// 最多返回多少条最近回执
        #[arg(long, default_value_t = 5)]
        limit: i64,
    },
    /// 打开公众号浏览器登录，供后续自动化复用登录态
    #[command(name = "report-login")]
    ReportLogin {
        /// 日报目标名称；为空时自动复用唯一日报目标
        #[arg(long, default_value = "")]
        report_name: String,
        /// 使用一次性隔离浏览器 profile，不复用本机持久登录态
        #[arg(long)]
        temporary_profile: bool,
    },
    /// 重试公众号浏览器自动化配置步骤
    #[command(name = "report-configure")]
    ReportConfigure {
        /// 日报目标名称；为空时自动复用唯一日报目标
        #[arg(long, default_value = "")]
        report_name: String,
        /// 用有头浏览器调试自动化步骤
        #[arg(long)]
        headed: bool,
        /// 使用一次性隔离浏览器 profile，不复用本机持久登录态
        #[arg(long)]
        temporary_profile: bool,
    },
    /// 顺序执行公众号登录与浏览器自动化重试
    #[command(name = "report-recover-automation")]
    ReportRecoverAutomation {
        /// 日报目标名称；为空时自动复用唯一日报目标
        #[arg(long, default_value = "")]
        report_name: String,
        /// 用有头浏览器调试自动化步骤
        #[arg(long)]
        headed: bool,
        /// 使用一次性隔离浏览器 profile，不复用本机持久登录态
        #[arg(long)]
        temporary_profile: bool,
    },
    /// 单独测试公众号预览步骤
    #[command(name = "report-preview")]
    ReportPreview {
        /// 日报目标名称；为空时自动复用唯一日报目标
        #[arg(long, default_value = "")]
        report_name: String,
        /// 用有头浏览器调试预览步骤
        #[arg(long)]
        headed: bool,
        /// 使用一次性隔离浏览器 profile，不复用本机持久登录态
        #[arg(long)]
        temporary_profile: bool,
    },
    /// 按已绑定公众号名称拉取文章列表
    #[command(name = "wechat-articles")]
    WechatArticles {
        /// 公众号名称或别名，必须先在 public_sources.wechat_accounts 中绑定 feed_url
        #[arg(long)]
        account_name: String,
        /// 最多返回多少篇文章
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// 按单篇公众号文章链接调用外部 helper 提取 markdown / 图片 / 元数据
    #[command(name = "wechat-article-url")]
    WechatArticleUrl {
        /// 单篇公众号文章链接，必须是 https://mp.weixin.qq.com/s/... 形式
        #[arg(long)]
        url: String,
        /// 可选输出目录；未传时使用 public_sources.wechat_article_helper_output_dir
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum WxCliCommand {
    /// Validate wx-cli readiness before touching a real WeChat group.
    Doctor {
        /// Parse a local wx-cli JSON file and include capture readiness signals.
        #[arg(long)]
        input: Option<PathBuf>,
        /// Maximum number of parsed messages to preview.
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Run wx_cli.poll_args once and save normalized messages for replay.
    Capture {
        /// Write normalized wx-cli messages to this JSON file.
        #[arg(long)]
        output: PathBuf,
    },
    /// Print the safe formal-test command sequence for a real WeChat group.
    TestPlan {
        /// Capture file used by doctor, dry-run, and handle-once replay steps.
        #[arg(long, default_value = "wx-output.json")]
        capture_file: PathBuf,
        /// Parse a captured wx-cli JSON file and auto-select one safe reply candidate when possible.
        #[arg(long)]
        input: Option<PathBuf>,
        /// Message id selected from doctor capture.reply_candidate_message_ids.
        #[arg(long)]
        message_id: Option<String>,
        /// Test chat id used by wx-cli send diagnostics.
        #[arg(long)]
        chat_id: Option<String>,
        /// Diagnostic text sent by wx-cli send dry-run and send steps.
        #[arg(long, default_value = "QunMind diagnostic message")]
        text: String,
        /// Print a shell script instead of JSON. Real-send steps stay commented.
        #[arg(long)]
        shell: bool,
    },
    /// Run wx_cli.poll_args once and print normalized messages.
    Poll {
        /// Parse a local wx-cli JSON file instead of invoking wx_cli.poll_args.
        #[arg(long)]
        input: Option<PathBuf>,
    },
    /// Poll once and preview reply decisions without storing, calling AI, or sending.
    DryRun {
        /// Parse a local wx-cli JSON file instead of invoking wx_cli.poll_args.
        #[arg(long)]
        input: Option<PathBuf>,
        /// Only inspect the matching message id.
        #[arg(long)]
        message_id: Option<String>,
        /// Maximum number of messages to inspect.
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Poll once and pass normalized messages through the real bot pipeline.
    HandleOnce {
        /// Parse a local wx-cli JSON file instead of invoking wx_cli.poll_args.
        #[arg(long)]
        input: Option<PathBuf>,
        /// Only process the matching message id.
        #[arg(long)]
        message_id: Option<String>,
        /// Maximum number of messages to process; defaults low to avoid noisy real chats.
        #[arg(long, default_value_t = 1)]
        limit: usize,
        /// Run the real persistence and AI pipeline but suppress wx-cli replies.
        #[arg(long)]
        no_send: bool,
    },
    /// Send one diagnostic text message through wx_cli.send_args.
    Send {
        #[arg(long)]
        chat_id: String,
        #[arg(long)]
        text: String,
        /// Render the wx-cli send command without executing it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Extract and cache the SQLCipher database key via LLDB (requires WeChat restart).
    ///
    /// Use this when memory scan is blocked by SIP / missing entitlement.
    /// The key is saved to ~/.qunmind/db_keys.cache and reused in future polls.
    KeysExtract,
    /// Show whether a key cache file exists and how many keys it contains.
    KeysStatus,
    /// Read-only: resolve keys, decrypt the newest message shard, and print its real table schema.
    ///
    /// Touches no AI / PostgreSQL / send paths. Use it to locate where the receive
    /// pipeline breaks: key extraction, decryption, or the WeChat 4.x table schema.
    Probe,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Args {
        match Args::try_parse_from(args) {
            Ok(args) => args,
            Err(err) => panic!("args: {err}"),
        }
    }

    #[test]
    fn parses_wx_cli_poll_command() {
        let args = parse_args(&["qunmind", "--config", "local.toml", "wx-cli", "poll"]);

        assert_eq!(args.config, PathBuf::from("local.toml"));
        assert!(matches!(
            args.command,
            Some(CliCommand::WxCli {
                command: WxCliCommand::Poll { input: None }
            })
        ));
    }

    #[test]
    fn parses_wx_cli_doctor_command() {
        let args = parse_args(&[
            "qunmind",
            "wx-cli",
            "doctor",
            "--input",
            "wx-output.json",
            "--limit",
            "3",
        ]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::Doctor {
                        input: Some(input),
                        limit,
                    },
            }) => {
                assert_eq!(input, PathBuf::from("wx-output.json"));
                assert_eq!(limit, 3);
            }
            _ => panic!("wx-cli doctor command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_capture_command() {
        let args = parse_args(&["qunmind", "wx-cli", "capture", "--output", "wx-output.json"]);

        match args.command {
            Some(CliCommand::WxCli {
                command: WxCliCommand::Capture { output },
            }) => assert_eq!(output, PathBuf::from("wx-output.json")),
            _ => panic!("wx-cli capture command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_test_plan_command() {
        let args = parse_args(&[
            "qunmind",
            "wx-cli",
            "test-plan",
            "--capture-file",
            "wx-output.json",
            "--message-id",
            "m-1",
            "--chat-id",
            "room@chatroom",
            "--text",
            "hello",
        ]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::TestPlan {
                        capture_file,
                        input,
                        message_id,
                        chat_id,
                        text,
                        shell,
                    },
            }) => {
                assert_eq!(capture_file, PathBuf::from("wx-output.json"));
                assert!(input.is_none());
                assert_eq!(message_id.as_deref(), Some("m-1"));
                assert_eq!(chat_id.as_deref(), Some("room@chatroom"));
                assert_eq!(text, "hello");
                assert!(!shell);
            }
            _ => panic!("wx-cli test-plan command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_test_plan_input_file() {
        let args = parse_args(&[
            "qunmind",
            "wx-cli",
            "test-plan",
            "--input",
            "wx-output.json",
        ]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::TestPlan {
                        capture_file,
                        input: Some(input),
                        message_id,
                        chat_id,
                        text,
                        shell,
                    },
            }) => {
                assert_eq!(capture_file, PathBuf::from("wx-output.json"));
                assert_eq!(input, PathBuf::from("wx-output.json"));
                assert!(message_id.is_none());
                assert!(chat_id.is_none());
                assert_eq!(text, "QunMind diagnostic message");
                assert!(!shell);
            }
            _ => panic!("wx-cli test-plan input command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_test_plan_shell_option() {
        let args = parse_args(&["qunmind", "wx-cli", "test-plan", "--shell"]);

        match args.command {
            Some(CliCommand::WxCli {
                command: WxCliCommand::TestPlan { shell, .. },
            }) => assert!(shell),
            _ => panic!("wx-cli test-plan shell option should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_dry_run_command() {
        let args = parse_args(&["qunmind", "wx-cli", "dry-run", "--limit", "5"]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::DryRun {
                        input: None,
                        message_id: None,
                        limit,
                    },
            }) => assert_eq!(limit, 5),
            _ => panic!("wx-cli dry-run command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_input_file_options() {
        let args = parse_args(&["qunmind", "wx-cli", "dry-run", "--input", "wx-output.json"]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::DryRun {
                        input: Some(input),
                        message_id: None,
                        limit,
                    },
            }) => {
                assert_eq!(input, PathBuf::from("wx-output.json"));
                assert_eq!(limit, 10);
            }
            _ => panic!("wx-cli dry-run command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_dry_run_message_id_option() {
        let args = parse_args(&[
            "qunmind",
            "wx-cli",
            "dry-run",
            "--input",
            "wx-output.json",
            "--message-id",
            "m-1",
        ]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::DryRun {
                        input: Some(input),
                        message_id: Some(message_id),
                        limit,
                    },
            }) => {
                assert_eq!(input, PathBuf::from("wx-output.json"));
                assert_eq!(message_id, "m-1");
                assert_eq!(limit, 10);
            }
            _ => panic!("wx-cli dry-run command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_poll_input_file_option() {
        let args = parse_args(&["qunmind", "wx-cli", "poll", "--input", "wx-output.json"]);

        match args.command {
            Some(CliCommand::WxCli {
                command: WxCliCommand::Poll { input: Some(input) },
            }) => assert_eq!(input, PathBuf::from("wx-output.json")),
            _ => panic!("wx-cli poll command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_handle_once_command() {
        let args = parse_args(&["qunmind", "wx-cli", "handle-once", "--limit", "3"]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::HandleOnce {
                        input: None,
                        message_id: None,
                        limit,
                        no_send,
                    },
            }) => {
                assert_eq!(limit, 3);
                assert!(!no_send);
            }
            _ => panic!("wx-cli handle-once command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_handle_once_input_file_option() {
        let args = parse_args(&[
            "qunmind",
            "wx-cli",
            "handle-once",
            "--input",
            "wx-output.json",
            "--message-id",
            "m-2",
            "--limit",
            "2",
        ]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::HandleOnce {
                        input: Some(input),
                        message_id: Some(message_id),
                        limit,
                        no_send,
                    },
            }) => {
                assert_eq!(input, PathBuf::from("wx-output.json"));
                assert_eq!(message_id, "m-2");
                assert_eq!(limit, 2);
                assert!(!no_send);
            }
            _ => panic!("wx-cli handle-once command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_handle_once_no_send_option() {
        let args = parse_args(&[
            "qunmind",
            "wx-cli",
            "handle-once",
            "--input",
            "wx-output.json",
            "--message-id",
            "m-2",
            "--no-send",
        ]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::HandleOnce {
                        input: Some(input),
                        message_id: Some(message_id),
                        limit,
                        no_send,
                    },
            }) => {
                assert_eq!(input, PathBuf::from("wx-output.json"));
                assert_eq!(message_id, "m-2");
                assert_eq!(limit, 1);
                assert!(no_send);
            }
            _ => panic!("wx-cli handle-once command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_send_command() {
        let args = parse_args(&[
            "qunmind",
            "wx-cli",
            "send",
            "--chat-id",
            "room@chatroom",
            "--text",
            "hello",
        ]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::Send {
                        chat_id,
                        text,
                        dry_run,
                    },
            }) => {
                assert_eq!(chat_id, "room@chatroom");
                assert_eq!(text, "hello");
                assert!(!dry_run);
            }
            _ => panic!("wx-cli send command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_send_dry_run_command() {
        let args = parse_args(&[
            "qunmind",
            "wx-cli",
            "send",
            "--chat-id",
            "room@chatroom",
            "--text",
            "hello",
            "--dry-run",
        ]);

        match args.command {
            Some(CliCommand::WxCli {
                command:
                    WxCliCommand::Send {
                        chat_id,
                        text,
                        dry_run,
                    },
            }) => {
                assert_eq!(chat_id, "room@chatroom");
                assert_eq!(text, "hello");
                assert!(dry_run);
            }
            _ => panic!("wx-cli send command should parse"),
        }
    }

    #[test]
    fn parses_wx_cli_probe_command() {
        let args = parse_args(&["qunmind", "wx-cli", "probe"]);

        assert!(matches!(
            args.command,
            Some(CliCommand::WxCli {
                command: WxCliCommand::Probe
            })
        ));
    }

    #[test]
    fn parses_daily_report_command() {
        let args = parse_args(&[
            "qunmind",
            "daily-report",
            "--output",
            "/tmp/daily.md",
            "--report-name",
            "技术群日报",
            "--hours",
            "48",
            "--publish",
        ]);

        match args.command {
            Some(CliCommand::DailyReport {
                output,
                report_name,
                hours,
                publish,
            }) => {
                assert_eq!(output, PathBuf::from("/tmp/daily.md"));
                assert_eq!(report_name, "技术群日报");
                assert_eq!(hours, 48);
                assert!(publish);
            }
            _ => panic!("daily-report command should parse"),
        }
    }

    #[test]
    fn parses_daily_report_default_hours() {
        let args = parse_args(&["qunmind", "daily-report", "--output", "/tmp/daily.md"]);

        match args.command {
            Some(CliCommand::DailyReport {
                report_name,
                hours,
                publish,
                ..
            }) => {
                assert_eq!(report_name, "");
                assert_eq!(hours, 24);
                assert!(!publish);
            }
            _ => panic!("daily-report command should parse"),
        }
    }

    #[test]
    fn parses_publish_history_command() {
        let args = parse_args(&[
            "qunmind",
            "publish-history",
            "--report-name",
            "技术群日报",
            "--limit",
            "3",
        ]);

        match args.command {
            Some(CliCommand::PublishHistory { report_name, limit }) => {
                assert_eq!(report_name, "技术群日报");
                assert_eq!(limit, 3);
            }
            _ => panic!("publish-history command should parse"),
        }
    }

    #[test]
    fn parses_report_status_command() {
        let args = parse_args(&[
            "qunmind",
            "report-status",
            "--report-name",
            "技术群日报",
            "--limit",
            "2",
        ]);

        match args.command {
            Some(CliCommand::ReportStatus { report_name, limit }) => {
                assert_eq!(report_name, "技术群日报");
                assert_eq!(limit, 2);
            }
            _ => panic!("report-status command should parse"),
        }
    }

    #[test]
    fn parses_report_login_command() {
        let args = parse_args(&[
            "qunmind",
            "report-login",
            "--report-name",
            "技术群日报",
            "--temporary-profile",
        ]);

        match args.command {
            Some(CliCommand::ReportLogin {
                report_name,
                temporary_profile,
            }) => {
                assert_eq!(report_name, "技术群日报");
                assert!(temporary_profile);
            }
            _ => panic!("report-login command should parse"),
        }
    }

    #[test]
    fn parses_report_configure_command() {
        let args = parse_args(&[
            "qunmind",
            "report-configure",
            "--report-name",
            "技术群日报",
            "--headed",
            "--temporary-profile",
        ]);

        match args.command {
            Some(CliCommand::ReportConfigure {
                report_name,
                headed,
                temporary_profile,
            }) => {
                assert_eq!(report_name, "技术群日报");
                assert!(headed);
                assert!(temporary_profile);
            }
            _ => panic!("report-configure command should parse"),
        }
    }

    #[test]
    fn parses_report_recover_automation_command() {
        let args = parse_args(&[
            "qunmind",
            "report-recover-automation",
            "--report-name",
            "技术群日报",
            "--headed",
            "--temporary-profile",
        ]);

        match args.command {
            Some(CliCommand::ReportRecoverAutomation {
                report_name,
                headed,
                temporary_profile,
            }) => {
                assert_eq!(report_name, "技术群日报");
                assert!(headed);
                assert!(temporary_profile);
            }
            _ => panic!("report-recover-automation command should parse"),
        }
    }

    #[test]
    fn parses_report_preview_command() {
        let args = parse_args(&[
            "qunmind",
            "report-preview",
            "--report-name",
            "技术群日报",
            "--headed",
            "--temporary-profile",
        ]);

        match args.command {
            Some(CliCommand::ReportPreview {
                report_name,
                headed,
                temporary_profile,
            }) => {
                assert_eq!(report_name, "技术群日报");
                assert!(headed);
                assert!(temporary_profile);
            }
            _ => panic!("report-preview command should parse"),
        }
    }

    #[test]
    fn parses_wechat_articles_command() {
        let args = parse_args(&[
            "qunmind",
            "wechat-articles",
            "--account-name",
            "寻月隐君",
            "--limit",
            "20",
        ]);

        match args.command {
            Some(CliCommand::WechatArticles {
                account_name,
                limit,
            }) => {
                assert_eq!(account_name, "寻月隐君");
                assert_eq!(limit, 20);
            }
            _ => panic!("wechat-articles command should parse"),
        }
    }

    #[test]
    fn parses_wechat_article_url_command() {
        let args = parse_args(&[
            "qunmind",
            "wechat-article-url",
            "--url",
            "https://mp.weixin.qq.com/s/example",
            "--output-dir",
            "/tmp/wechat-helper",
        ]);

        match args.command {
            Some(CliCommand::WechatArticleUrl { url, output_dir }) => {
                assert_eq!(url, "https://mp.weixin.qq.com/s/example");
                assert_eq!(output_dir, Some(PathBuf::from("/tmp/wechat-helper")));
            }
            _ => panic!("wechat-article-url command should parse"),
        }
    }
}
