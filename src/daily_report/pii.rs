//! PII / 隐私边界扫描。
//!
//! 移植自 wx-cli `scripts/validation/check-roast-boundaries.mjs` 的硬规则思路：
//! 命中强 PII（手机/邮箱/身份证/车牌/精确门牌）即报告并给出脱敏串；
//! 银行卡号按软规则处理（Web3 日报里 16-19 位大整数很常见，避免误伤）。
//!
//! 设计取舍：QunMind 日报以 AI/Web3 公开新闻为主，真实 PII 极少，但我们的定位
//! 也包含微信群消息归一化与持久化，因此把 PII 扫描作为可复用的内容安全边界：
//! 硬 PII 在 `lint` 中以 Error 暴露（阻断把含 PII 的日报发布出去），并可配合
//! [`redact_pii`] 做自动脱敏。

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiKind {
    Phone,
    Email,
    IdCard,
    BankCard,
    LicensePlate,
    PreciseAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiiFinding {
    pub kind: PiiKind,
    pub matched: String,
    pub redacted: String,
    /// 原始文本中的字节偏移，便于定位与替换
    pub start: usize,
    pub end: usize,
}

impl PiiFinding {
    /// lint issue code，格式 `privacy_pii_<kind>`
    pub fn code(&self) -> &'static str {
        match self.kind {
            PiiKind::Phone => "privacy_pii_phone",
            PiiKind::Email => "privacy_pii_email",
            PiiKind::IdCard => "privacy_pii_id_card",
            PiiKind::BankCard => "privacy_pii_bank_card",
            PiiKind::LicensePlate => "privacy_pii_license_plate",
            PiiKind::PreciseAddress => "privacy_pii_precise_address",
        }
    }

    pub fn label(&self) -> &'static str {
        match self.kind {
            PiiKind::Phone => "手机号",
            PiiKind::Email => "邮箱",
            PiiKind::IdCard => "身份证号",
            PiiKind::BankCard => "银行卡号",
            PiiKind::LicensePlate => "车牌号",
            PiiKind::PreciseAddress => "精确门牌/楼栋",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiSeverity {
    Error,
    Warn,
}

impl PiiFinding {
    /// 在 lint 中的严重级别：银行卡号为软警告，其余为硬错误。
    pub fn severity(&self) -> PiiSeverity {
        match self.kind {
            PiiKind::BankCard => PiiSeverity::Warn,
            _ => PiiSeverity::Error,
        }
    }
}

fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}")
            .expect("compile email regex")
    })
}

fn license_plate_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"[京津沪渝冀豫云辽黑湘皖鲁新苏浙赣鄂桂甘晋蒙陕吉闽贵粤川青藏琼宁][A-Z][A-Z0-9]{5}",
        )
        .expect("compile license plate regex")
    })
}

fn precise_address_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\d+号\d+楼|\d+栋\d+单元").expect("compile precise address regex")
    })
}

fn url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?:https?://|www\.)[^\s<>"'\]}）]+"#).expect("compile url regex")
    })
}

fn inline_code_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`[^`\n]*`").expect("compile inline code regex"))
}

fn fence_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"```[\s\S]*?```").expect("compile fence regex"))
}

/// 跳过 URL 与代码块，避免把其中合法的長数字/样例卡号误判为 PII。
fn skip_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for re in [url_re(), inline_code_re(), fence_re()] {
        for m in re.find_iter(text) {
            spans.push((m.start(), m.end()));
        }
    }
    spans
}

fn intersects(range: std::ops::Range<usize>, spans: &[(usize, usize)]) -> bool {
    spans
        .iter()
        .any(|(s, e)| range.start < *e && range.end > *s)
}

fn push_finding(
    findings: &mut Vec<PiiFinding>,
    kind: PiiKind,
    matched: &str,
    spans: &[(usize, usize)],
    start: usize,
    end: usize,
    redact: impl FnOnce() -> String,
) {
    if intersects(start..end, spans) {
        return;
    }
    findings.push(PiiFinding {
        kind,
        matched: matched.to_string(),
        redacted: redact(),
        start,
        end,
    });
}

/// 扫描文本中的强 PII，返回所有命中（已排除 URL / 代码块）。
pub fn scan_pii(text: &str) -> Vec<PiiFinding> {
    let spans = skip_spans(text);
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    let n = bytes.len();
    let mut i = 0;

    // 连续数字类 PII：手机 / 身份证 / 银行卡
    while i < n {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < n && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let end = i;
            let run = &text[start..end];
            let len = run.len();
            let prev_ok = start == 0 || !bytes[start - 1].is_ascii_digit();
            let next_ok = end == n || !bytes[end].is_ascii_digit();

            // 身份证：17 位数字 + 校验位 X/x（在数字串处断开，需并入尾字符）
            let next_char = text[end..].chars().next();
            let is_id_17x = len == 17 && matches!(next_char, Some('X') | Some('x'));

            if len == 11 && run.starts_with('1') && (b'3'..=b'9').contains(&run.as_bytes()[1]) {
                push_finding(
                    &mut findings,
                    PiiKind::Phone,
                    run,
                    &spans,
                    start,
                    end,
                    || format!("{}****{}", &run[..3], &run[7..]),
                );
            } else if is_id_17x {
                let full = &text[start..end + 1];
                push_finding(
                    &mut findings,
                    PiiKind::IdCard,
                    full,
                    &spans,
                    start,
                    end + 1,
                    || format!("{}{}{}", &full[..3], "*".repeat(11), &full[14..]),
                );
            } else if len == 18 {
                push_finding(
                    &mut findings,
                    PiiKind::IdCard,
                    run,
                    &spans,
                    start,
                    end,
                    || format!("{}{}{}", &run[..3], "*".repeat(11), &run[14..]),
                );
            } else if (16..=19).contains(&len) && prev_ok && next_ok {
                push_finding(
                    &mut findings,
                    PiiKind::BankCard,
                    run,
                    &spans,
                    start,
                    end,
                    || format!("{}****{}", &run[..4], &run[len - 4..]),
                );
            }
        } else {
            i += 1;
        }
    }

    for m in email_re().find_iter(text) {
        if intersects(m.range(), &spans) {
            continue;
        }
        let s = m.as_str();
        let at = s.find('@').expect("email regex guarantees @");
        let local = &s[..at];
        let domain = &s[at..];
        let visible = if local.len() > 2 {
            &local[..2]
        } else {
            &local[..1]
        };
        push_finding(
            &mut findings,
            PiiKind::Email,
            s,
            &spans,
            m.start(),
            m.end(),
            || format!("{}***{}", visible, domain),
        );
    }

    for m in license_plate_re().find_iter(text) {
        if intersects(m.range(), &spans) {
            continue;
        }
        let s = m.as_str();
        push_finding(
            &mut findings,
            PiiKind::LicensePlate,
            s,
            &spans,
            m.start(),
            m.end(),
            || {
                let chars: Vec<char> = s.chars().collect();
                let head: String = chars.iter().take(2).collect();
                let tail: String = chars
                    .iter()
                    .rev()
                    .take(2)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                format!("{}***{}", head, tail)
            },
        );
    }

    for m in precise_address_re().find_iter(text) {
        if intersects(m.range(), &spans) {
            continue;
        }
        let s = m.as_str();
        push_finding(
            &mut findings,
            PiiKind::PreciseAddress,
            s,
            &spans,
            m.start(),
            m.end(),
            || s.replace(|c: char| c.is_ascii_digit(), "*"),
        );
    }

    findings.sort_by_key(|f| f.start);
    findings
}

/// 用各命中的脱敏串替换原文（从后往前替换以保证偏移有效）。
pub fn redact_pii(text: &str, findings: &[PiiFinding]) -> String {
    let mut out = text.to_string();
    for f in findings.iter().rev() {
        out.replace_range(f.start..f.end, &f.redacted);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_phone_and_redacts() {
        let text = "我的手机号是13812345678，请加我";
        let findings = scan_pii(text);
        let phone = findings.iter().find(|f| f.kind == PiiKind::Phone).unwrap();
        assert_eq!(phone.matched, "13812345678");
        assert_eq!(phone.redacted, "138****5678");
        assert_eq!(
            redact_pii(text, &findings),
            "我的手机号是138****5678，请加我"
        );
    }

    #[test]
    fn detects_email_and_id_card() {
        let text = "联系 alice@example.com 或身份证 11010519900307891X 的人";
        let findings = scan_pii(text);
        assert!(
            findings
                .iter()
                .any(|f| f.kind == PiiKind::Email && f.matched == "alice@example.com")
        );
        let id = findings
            .iter()
            .find(|f| f.kind == PiiKind::IdCard)
            .expect("id card found");
        assert_eq!(id.redacted, "110***********891X");
    }

    #[test]
    fn skips_urls_and_code() {
        let text = "详见 https://example.com/13812345678/path 与 `secret 13812345678` 内联代码";
        let findings = scan_pii(text);
        assert!(
            findings.iter().all(|f| f.kind != PiiKind::Phone),
            "URL 与代码块里的数字不应被判为手机号"
        );
    }

    #[test]
    fn bank_card_is_warn_not_error() {
        let text = "卡号 6222021234567890123 被盗刷";
        let findings = scan_pii(text);
        let bank = findings
            .iter()
            .find(|f| f.kind == PiiKind::BankCard)
            .expect("bank card found");
        assert_eq!(bank.severity(), PiiSeverity::Warn);
        assert_eq!(bank.redacted, "6222****0123");
    }

    #[test]
    fn license_plate_detected() {
        let text = "京A12345 在小区门口";
        let findings = scan_pii(text);
        let plate = findings
            .iter()
            .find(|f| f.kind == PiiKind::LicensePlate)
            .expect("plate found");
        assert_eq!(plate.redacted, "京A***45");
    }

    #[test]
    fn big_web3_integer_not_flagged_as_bank_card() {
        // 20 位（超出 16-19 边界）不应命中银行卡；19 位边界值会命中
        let text = "gas 10000000000000000000 wei";
        let findings = scan_pii(text);
        assert!(findings.iter().all(|f| f.kind != PiiKind::BankCard));
    }
}
