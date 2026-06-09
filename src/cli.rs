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
    },
    /// Send one diagnostic text message through wx_cli.send_args.
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
                    },
            }) => assert_eq!(limit, 3),
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
                    },
            }) => {
                assert_eq!(input, PathBuf::from("wx-output.json"));
                assert_eq!(message_id, "m-2");
                assert_eq!(limit, 2);
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
                command: WxCliCommand::Send { chat_id, text },
            }) => {
                assert_eq!(chat_id, "room@chatroom");
                assert_eq!(text, "hello");
            }
            _ => panic!("wx-cli send command should parse"),
        }
    }
}
