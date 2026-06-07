use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "qunmind", about = "微信群 AI 群智中枢")]
pub struct Args {
    /// 配置文件路径
    #[arg(short, long, default_value = "config.toml", global = true)]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// 单独诊断 wx-cli 收发命令，不启动机器人主循环
    #[command(name = "wx-cli")]
    WxCli {
        #[command(subcommand)]
        command: WxCliCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum WxCliCommand {
    /// 执行一次 wx_cli.poll_args 并输出解析后的消息
    Poll {
        /// 从本地 wx-cli JSON 文件解析，不实际调用 wx_cli.poll_args
        #[arg(long)]
        input: Option<PathBuf>,
    },
    /// 执行一次 poll，只预检哪些消息会触发回复，不保存、不调用 AI、不发送
    DryRun {
        /// 从本地 wx-cli JSON 文件解析，不实际调用 wx_cli.poll_args
        #[arg(long)]
        input: Option<PathBuf>,
        /// 本次最多预检多少条消息
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// 执行一次 poll，并把解析后的消息交给机器人链路处理
    HandleOnce {
        /// 本次最多处理多少条消息，默认只处理 1 条，避免联调时刷屏
        #[arg(long, default_value_t = 1)]
        limit: usize,
    },
    /// 通过 wx_cli.send_args 向指定会话发送一条文本
    Send {
        #[arg(long)]
        chat_id: String,
        #[arg(long)]
        text: String,
    },
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
    fn parses_wx_cli_dry_run_command() {
        let args = parse_args(&["qunmind", "wx-cli", "dry-run", "--limit", "5"]);

        match args.command {
            Some(CliCommand::WxCli {
                command: WxCliCommand::DryRun { input: None, limit },
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
                command: WxCliCommand::HandleOnce { limit },
            }) => assert_eq!(limit, 3),
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
                command: WxCliCommand::Send { chat_id, text },
            }) => {
                assert_eq!(chat_id, "room@chatroom");
                assert_eq!(text, "hello");
            }
            _ => panic!("wx-cli send command should parse"),
        }
    }
}
