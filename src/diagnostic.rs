mod doctor;
mod dry_run;
mod formal_test;
mod pipeline;
mod support;

pub use doctor::{wx_cli_capture_report, wx_cli_doctor_report};
pub use dry_run::{
    wx_cli_dry_run_item, wx_cli_dry_run_message_id_guard_report,
    wx_cli_dry_run_message_id_not_found_report, wx_cli_dry_run_message_id_not_unique_report,
    wx_cli_dry_run_report, wx_cli_handle_once_message_id_guard_report,
    wx_cli_handle_once_message_id_not_found_report,
    wx_cli_handle_once_message_id_not_unique_report, wx_cli_handle_once_message_id_required_report,
    wx_cli_handle_once_report, wx_cli_handle_once_selected_message_not_group_report,
    wx_cli_handle_once_selected_message_would_not_reply_report,
};
pub use formal_test::{wx_cli_formal_test_plan, wx_cli_formal_test_plan_shell_script};
pub use pipeline::wx_cli_handle_once_pipeline;
pub use support::{select_wx_cli_messages, wx_cli_message_id_match_count, wx_cli_message_ids};

#[cfg(test)]
use support::{effective_bot_config, text_preview, wx_cli_dry_run_decision};

#[cfg(test)]
mod tests;
