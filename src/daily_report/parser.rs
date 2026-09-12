use std::collections::HashSet;

use crate::daily_report::types::ReportJson;

pub(super) fn parse_report_json(raw: &str) -> ReportJson {
    let json_str = extract_json(raw);

    // Fast path: normal parse
    if let Ok(r) = serde_json::from_str(json_str) {
        return r;
    }

    // Slow path: DeepSeek occasionally outputs duplicate top-level keys
    // (e.g. two "summary" fields). Remove duplicates, keeping the last
    // occurrence of each key.
    let deduped = remove_duplicate_keys(json_str);
    match serde_json::from_str(&deduped) {
        Ok(r) => r,
        Err(_) => repair_and_parse(raw),
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

fn repair_and_parse(raw: &str) -> ReportJson {
    let repaired = repair_common_json_issues(extract_json(raw));

    if let Ok(report) = serde_json::from_str::<ReportJson>(&repaired) {
        return report;
    }

    let deduped = remove_duplicate_keys(&repaired);
    match serde_json::from_str(&deduped) {
        Ok(report) => report,
        Err(e) => {
            tracing::warn!(error = %e, raw_len = raw.len(), "AI JSON 解析失败，使用空报告");
            ReportJson::default()
        }
    }
}

fn repair_common_json_issues(input: &str) -> String {
    let mut repaired = strip_invalid_control_chars(input);
    repaired = strip_trailing_commas(&repaired);
    repaired = strip_parser_garbage(&repaired);
    repaired
}

fn strip_invalid_control_chars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;

    for ch in input.chars() {
        if in_string {
            if escape {
                out.push(ch);
                escape = false;
                continue;
            }
            match ch {
                '\\' => {
                    out.push(ch);
                    escape = true;
                }
                '"' => {
                    out.push(ch);
                    in_string = false;
                }
                '\u{0000}'..='\u{001f}' => {
                    if matches!(ch, '\n' | '\r' | '\t') {
                        out.push(' ');
                    }
                }
                _ => out.push(ch),
            }
            continue;
        }

        match ch {
            '"' => {
                out.push(ch);
                in_string = true;
            }
            '\u{0000}'..='\u{0008}' | '\u{000b}' | '\u{000c}' | '\u{000e}'..='\u{001f}' => {}
            _ => out.push(ch),
        }
    }

    out
}

fn strip_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            ',' => {
                let mut lookahead = chars.clone();
                while let Some(next) = lookahead.peek() {
                    if next.is_whitespace() {
                        lookahead.next();
                        continue;
                    }
                    if *next == '}' || *next == ']' {
                        break;
                    }
                    out.push(ch);
                    break;
                }
            }
            _ => out.push(ch),
        }
    }

    out
}

fn strip_parser_garbage(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '请' if chars.clone().collect::<String>().starts_with("注意：") => break,
            _ => out.push(ch),
        }
    }

    out
}

/// Remove duplicate top-level keys from a JSON object string.
/// Keeps the *last* occurrence of each key.
fn remove_duplicate_keys(json: &str) -> String {
    let s = json.trim();
    let inner = match s.strip_prefix('{').and_then(|t| t.strip_suffix('}')) {
        Some(v) => v,
        None => return s.to_string(),
    };

    let segments = top_level_segments(inner);
    let mut seen = HashSet::with_capacity(segments.len());
    let mut unique: Vec<&str> = Vec::with_capacity(segments.len());

    // Walk in reverse, keeping last occurrence of each key
    for seg in segments.iter().rev() {
        if let Some(key) = segment_key(seg)
            && seen.insert(key)
        {
            unique.push(seg);
        }
    }
    unique.reverse();

    format!("{{{}}}", unique.join(","))
}

/// Split the inner content of a JSON object (already stripped of {}) into
/// top-level comma-separated segments, respecting nesting.
fn top_level_segments(inner: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut seg_start = 0;

    for (i, ch) in inner.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '{' | '[' if !in_string => depth += 1,
            '}' | ']' if !in_string => {
                depth = (depth - 1).max(0);
            }
            ',' if !in_string && depth == 0 => {
                segments.push(inner[seg_start..i].trim());
                seg_start = i + 1;
            }
            _ => {}
        }
    }

    let last = inner[seg_start..].trim();
    if !last.is_empty() {
        segments.push(last);
    }

    segments
}

/// Extract the key string from a `"key":value` segment.
fn segment_key(seg: &str) -> Option<String> {
    let s = seg.trim_start();
    if !s.starts_with('"') {
        return None;
    }
    let mut escape = false;
    for (i, ch) in s[1..].char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' => escape = true,
            '"' => {
                // The key ends at position i+1 (offset 1 for the skipped opening quote)
                return Some(s[1..i + 1].to_string());
            }
            _ => {}
        }
    }
    None
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

    #[test]
    fn handles_duplicate_keys() {
        // DeepSeek sometimes emits duplicate "summary" fields
        let raw = r#"{"title_hint":"测试","intro":"描述","summary":"第一条","summary":"最后一条"}"#;
        let report = parse_report_json(raw);
        assert_eq!(report.intro, "描述");
        assert_eq!(report.summary, "最后一条"); // keeps last occurrence
    }

    #[test]
    fn remove_duplicate_keys_keeps_last() {
        let input = r#"{"a":1,"b":2,"a":3}"#;
        let result = remove_duplicate_keys(input);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["a"], 3);
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn remove_duplicate_keys_with_nested_objects() {
        let input = r#"{"a":1,"b":{"x":1},"c":[1,2],"b":{"y":2}}"#;
        let result = remove_duplicate_keys(input);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["b"], serde_json::json!({"y": 2}));
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn remove_duplicate_keys_handles_commas_in_strings() {
        let input = r#"{"a":"hello, world","b":2,"a":3}"#;
        let result = remove_duplicate_keys(input);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["a"], 3);
    }

    #[test]
    fn strips_trailing_commas_before_parse() {
        let raw = r#"{"intro":"hello","ai_items":[{"title":"a","url":"https://a","comment":"b","source":"c","points":1,}],"summary":"ok",}"#;
        let report = parse_report_json(raw);
        assert_eq!(report.intro, "hello");
        assert_eq!(report.summary, "ok");
        assert_eq!(report.ai_items.len(), 1);
    }

    #[test]
    fn strips_parser_garbage_suffix() {
        let raw = r#"{"focus_text":"主线","summary":"总结"}请注意：如果需要全部条目聚合成单一摘要，当前文本含多条新闻"#;
        let report = parse_report_json(raw);
        assert_eq!(report.focus_text, "主线");
        assert_eq!(report.summary, "总结");
    }

    #[test]
    fn strips_invalid_control_chars_inside_json_strings() {
        let raw = "{\n\"intro\":\"第一行\n第二行\",\n\"summary\":\"保留\t可读性\"\n}";
        let report = parse_report_json(raw);
        assert_eq!(report.intro, "第一行 第二行");
        assert_eq!(report.summary, "保留 可读性");
    }
}
