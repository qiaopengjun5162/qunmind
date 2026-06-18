use crate::daily_report::types::ReportJson;

pub(super) fn parse_report_json(raw: &str) -> ReportJson {
    let json_str = extract_json(raw);
    match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, raw_len = raw.len(), "AI JSON 解析失败，使用空报告");
            ReportJson::default()
        }
    }
}

fn extract_json(raw: &str) -> &str {
    let s = raw.trim();
    let s = if let Some(inner) = s.strip_prefix("```json").or_else(|| s.strip_prefix("```")) {
        inner.trim_start()
    } else {
        s
    };
    let s = if let Some(inner) = s.strip_suffix("```") {
        inner.trim_end()
    } else {
        s
    };
    let start = s.find('{').unwrap_or(0);
    let end = s.rfind('}').map(|i| i + 1).unwrap_or(s.len());
    &s[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerates_code_fence() {
        let raw = "```json\n{\"intro\":\"hello\",\"summary\":\"world\"}\n```";
        let report = parse_report_json(raw);
        assert_eq!(report.intro, "hello");
        assert_eq!(report.summary, "world");
    }

    #[test]
    fn returns_default_on_invalid_json() {
        let report = parse_report_json("not json at all");
        assert!(report.intro.is_empty());
        assert!(report.summary.is_empty());
    }

    #[test]
    fn bare_json_without_fence() {
        let raw = r#"{"title_hint":"测试标题","intro":"简介","summary":"总结"}"#;
        let report = parse_report_json(raw);
        assert_eq!(report.title_hint, "测试标题");
        assert_eq!(report.intro, "简介");
    }

    #[test]
    fn extracts_json_from_surrounding_text() {
        let raw = "Sure, here is the JSON:\n{\"intro\":\"hi\"}\nDone.";
        let report = parse_report_json(raw);
        assert_eq!(report.intro, "hi");
    }
}
