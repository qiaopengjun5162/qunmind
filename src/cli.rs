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
    Poll,
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

    #[test]
    fn parses_wx_cli_poll_command() {
        let args = Args::try_parse_from(["qunmind", "--config", "local.toml", "wx-cli", "poll"])
            .expect("args");

        assert_eq!(args.config, PathBuf::from("local.toml"));
        assert!(matches!(
            args.command,
            Some(CliCommand::WxCli {
                command: WxCliCommand::Poll
            })
        ));
    }

    #[test]
    fn parses_wx_cli_handle_once_command() {
        let args = Args::try_parse_from(["qunmind", "wx-cli", "handle-once", "--limit", "3"])
            .expect("args");

        match args.command {
            Some(CliCommand::WxCli {
                command: WxCliCommand::HandleOnce { limit },
            }) => assert_eq!(limit, 3),
            _ => panic!("expected wx-cli handle-once command"),
        }
    }

    #[test]
    fn parses_wx_cli_send_command() {
        let args = Args::try_parse_from([
            "qunmind",
            "wx-cli",
            "send",
            "--chat-id",
            "room@chatroom",
            "--text",
            "hello",
        ])
        .expect("args");

        match args.command {
            Some(CliCommand::WxCli {
                command: WxCliCommand::Send { chat_id, text },
            }) => {
                assert_eq!(chat_id, "room@chatroom");
                assert_eq!(text, "hello");
            }
            _ => panic!("expected wx-cli send command"),
        }
    }
}
