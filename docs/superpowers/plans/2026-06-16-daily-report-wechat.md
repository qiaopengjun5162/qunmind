# 科技日报 + 微信公众号自动发布 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现日报生成器(两段式AI pipeline) + moonpub 微信发布集成，全部在 qunmind 一个进程内完成。

**Architecture:** 新增 `DailyReportGenerator`(纯内容生成)和 `WechatPublisher`(moonpub subprocess 封装)两个独立模块。`DailyReportScheduler` 根据 `DailyReportTarget.output` 类型路由到群聊 Channel 或微信发布路径。所有配置由 `config.toml` 的 `[[schedule.daily_reports]]` 统一管理。

**Tech Stack:** Rust 2024, tokio, async-trait, serde, clap, cron, tracing

**Files:**
- Create: `src/daily_report.rs`
- Create: `src/wechat_publisher.rs`
- Modify: `src/config.rs` — `DailyReportConfig` 加 `output`/`wechat_bin`/`wechat_articles_dir`，`ScheduleConfig` 加 `scoring_prompt`
- Modify: `src/source/mod.rs` — `PublicNewsItem` 加 `ai_score`/`category`，新增 `ScoredItem`
- Modify: `src/scheduler/daily_report.rs` — `DailyReportTarget` 加 `output` 等字段，路由逻辑
- Modify: `src/cli.rs` — 新增 `DailyReport` 子命令
- Modify: `src/main.rs` — 处理 `DailyReport` CLI 命令
- Modify: `src/lib.rs` — 注册新模块

---

### Task 1: Config — 扩展数据结构

**Files:**
- Modify: `src/config.rs`
- Modify: `src/source/mod.rs`

**背景:** `DailyReportTarget` 目前是 `scheduler/daily_report.rs` 的私有 struct，从 `DailyReportConfig` 构建。需要把微信发布相关字段加到 `DailyReportConfig`，然后透传到 `DailyReportTarget`。

- [ ] **Step 1: 在 `DailyReportConfig` 加字段**

在 `src/config.rs` 的 `DailyReportConfig` struct 末尾追加三个字段：

```rust
// src/config.rs — DailyReportConfig
#[derive(Debug, Clone, Deserialize)]
pub struct DailyReportConfig {
    pub chat_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub cron: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub lookback_hours: Option<i64>,
    #[serde(default)]
    pub max_messages: Option<i64>,
    #[serde(default)]
    pub max_links: Option<i64>,
    // 以下为新增字段
    #[serde(default = "default_report_output")]
    pub output: String,
    #[serde(default = "default_wechat_bin")]
    pub wechat_bin: String,
    #[serde(default)]
    pub wechat_articles_dir: String,
}
```

在 `src/config.rs` 文件末尾的 default 函数区域追加：

```rust
fn default_report_output() -> String {
    "chat".to_string()
}

fn default_wechat_bin() -> String {
    "moonpub".to_string()
}
```

在已有的测试 `parses_wx_cli_hermes_and_groups` 中，第一个 `daily_reports` entry 已验证 `chat_id` 等字段 — 新增字段由于有 `#[serde(default)]` 会自动填充默认值，不需要改测试。

- [ ] **Step 2: 在 `ScheduleConfig` 加 scoring prompt**

```rust
// src/config.rs — ScheduleConfig，在 daily_report_max_links 后面加
    #[serde(default = "default_daily_report_scoring_prompt")]
    pub daily_report_scoring_prompt: String,
```

并在文件末尾加对应的 default 函数：

```rust
fn default_daily_report_scoring_prompt() -> String {
    "你是一个科技新闻评分员。对每条新闻从以下维度评分(0-10):\n\
     - 技术重要性\n\
     - 行业影响力\n\
     - 与 Rust/Web3/AI/ZKP 的相关性\n\
     返回纯 JSON 数组，不要包含其他文字:\n\
     [{\"title\": \"...\", \"score\": 7, \"category\": \"AI\", \"reason\": \"...\"}]"
        .to_string()
}
```

并在 `impl Default for ScheduleConfig` 的 `Default::default()` 实现中确保新字段被正确初始化（查看已有代码，`#[serde(default)]` 已处理）。

- [ ] **Step 3: 为 `Default for ScheduleConfig` 补上 `daily_report_scoring_prompt` 字段**

查看 `src/config.rs:489-501`，`impl Default for ScheduleConfig` 目前没有包含 `daily_report_scoring_prompt`。由于 `#[serde(default = "...")]` 已经处理反序列化默认值，但手动 `Default` 实现也需要更新：

```rust
impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            daily_report_cron: default_daily_report_cron(),
            daily_report_chat_id: String::new(),
            daily_report_prompt: default_daily_report_prompt(),
            daily_report_lookback_hours: default_daily_report_lookback_hours(),
            daily_report_max_messages: default_daily_report_max_messages(),
            daily_report_max_links: default_daily_report_max_links(),
            daily_reports: Vec::new(),
            daily_report_scoring_prompt: default_daily_report_scoring_prompt(),
        }
    }
}
```

- [ ] **Step 4: 在 `PublicNewsItem` 加 `ai_score` 和 `category` 字段**

修改 `src/source/mod.rs:17-24`：

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PublicNewsItem {
    pub source: String,
    pub title: String,
    pub url: String,
    pub score: Option<i64>,
    pub comments: Option<i64>,
    pub ai_score: Option<f64>,
    pub category: Option<String>,
}
```

改为 `PartialEq` (去掉 `Eq`)，因为 `f64` 不实现 `Eq`。

在测试中 `StaticSource` 创建的 `PublicNewsItem` 里需要补上两个新字段：

```rust
PublicNewsItem {
    // ... existing fields ...
    ai_score: None,
    category: None,
}
```

`src/source/mod.rs` 中的所有测试构造都需要加这两个字段。

- [ ] **Step 5: 跑测试确认编译通过**

```bash
cargo test -p qunmind -- config::tests
cargo test -p qunmind -- source::tests
```

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/source/mod.rs
git commit -m "feat(config): add output/wechat fields to DailyReportConfig and ai_score/category to PublicNewsItem"
```

---

### Task 2: `ScoredItem` + scoring prompt 构建

**Files:**
- Modify: `src/source/mod.rs`

**背景:** AI 评分后需要把评分结果和原始 item 关联。新增一个 `ScoredItem` struct。

- [ ] **Step 1: 新增 `ScoredItem` struct + `build_scoring_prompt` 函数**

在 `src/source/mod.rs` 中，`PublicNewsItem` 定义之后追加：

```rust
#[derive(Debug, Clone)]
pub struct ScoredItem {
    pub item: PublicNewsItem,
    pub ai_score: f64,
    pub category: String,
    pub reason: String,
}

/// 构建评分 prompt，AI 返回 JSON 数组
pub fn build_scoring_prompt(items: &[PublicNewsItem], scoring_prompt: &str) -> String {
    let mut prompt = String::from(scoring_prompt);
    prompt.push_str("\n\n新闻列表:\n");
    for item in items {
        prompt.push_str(&format!(
            "- [{}] {} ({})\n",
            item.source,
            item.title.replace('\n', " "),
            item.url
        ));
    }
    prompt
}

/// 解析 AI 返回的评分 JSON
pub fn parse_scoring_json(json: &str, items: &[PublicNewsItem]) -> Result<Vec<ScoredItem>> {
    use serde::Deserialize;
    use crate::error::QunMindError;

    #[derive(Deserialize)]
    struct ScoreEntry {
        title: String,
        score: f64,
        category: String,
        reason: String,
    }

    let entries: Vec<ScoreEntry> = serde_json::from_str(json).map_err(|e| {
        QunMindError::Ai(format!("评分 JSON 解析失败: {}", e))
    })?;

    Ok(entries
        .into_iter()
        .filter_map(|e| {
            items.iter().find(|i| i.title == e.title).map(|i| ScoredItem {
                item: i.clone(),
                ai_score: e.score,
                category: e.category,
                reason: e.reason,
            })
        })
        .collect())
}
```

- [ ] **Step 2: 写测试**

在 `src/source/mod.rs` 的 `mod tests` 块中追加：

```rust
#[test]
fn build_scoring_prompt_includes_items_and_custom_prompt() {
    let items = vec![PublicNewsItem {
        source: "HN".to_string(),
        title: "Rust 2026".to_string(),
        url: "https://example.com/rust".to_string(),
        score: None,
        comments: None,
        ai_score: None,
        category: None,
    }];
    let prompt = build_scoring_prompt(&items, "请评分");
    assert!(prompt.contains("请评分"));
    assert!(prompt.contains("Rust 2026"));
    assert!(prompt.contains("https://example.com/rust"));
}

#[test]
fn parse_scoring_json_matches_items_by_title() {
    let items = vec![
        PublicNewsItem {
            source: "HN".to_string(),
            title: "Rust 2026".to_string(),
            url: "https://example.com/rust".to_string(),
            score: None,
            comments: None,
            ai_score: None,
            category: None,
        },
        PublicNewsItem {
            source: "HN".to_string(),
            title: "AI news".to_string(),
            url: "https://example.com/ai".to_string(),
            score: None,
            comments: None,
            ai_score: None,
            category: None,
        },
    ];
    let json = r#"[
        {"title": "Rust 2026", "score": 9, "category": "Dev", "reason": "重要"},
        {"title": "AI news", "score": 5, "category": "AI", "reason": "一般"}
    ]"#;
    let scored = parse_scoring_json(json, &items).unwrap();
    assert_eq!(scored.len(), 2);
    assert_eq!(scored[0].ai_score, 9.0);
    assert_eq!(scored[1].ai_score, 5.0);
}

#[test]
fn parse_scoring_json_skips_unmatched_titles() {
    let items = vec![PublicNewsItem {
        source: "HN".to_string(),
        title: "Rust 2026".to_string(),
        url: "https://example.com/rust".to_string(),
        score: None,
        comments: None,
        ai_score: None,
        category: None,
    }];
    let json = r#"[
        {"title": "Rust 2026", "score": 9, "category": "Dev", "reason": "ok"},
        {"title": "No match", "score": 8, "category": "Dev", "reason": "ok"}
    ]"#;
    let scored = parse_scoring_json(json, &items).unwrap();
    assert_eq!(scored.len(), 1);
    assert_eq!(scored[0].item.title, "Rust 2026");
}

#[test]
fn parse_scoring_json_rejects_invalid_json() {
    let items = vec![];
    let err = parse_scoring_json("not json", &items).unwrap_err();
    assert!(err.to_string().contains("JSON"));
}
```

- [ ] **Step 3: 跑测试确认**

```bash
cargo test -p qunmind -- source::tests::build_scoring_prompt
cargo test -p qunmind -- source::tests::parse_scoring
```

- [ ] **Step 4: Commit**

```bash
git add src/source/mod.rs
git commit -m "feat(source): add ScoredItem, build_scoring_prompt, parse_scoring_json"
```

---

### Task 3: `DailyReportGenerator` — 两段式 AI Pipeline

**Files:**
- Create: `src/daily_report.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: 在 `src/lib.rs` 注册新模块**

```rust
// src/lib.rs — 在上方模块注册区加一行
pub mod daily_report;
```

- [ ] **Step 2: 创建 `src/daily_report.rs` — 先写测试**

```rust
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::ai::{AiClient, ChatMessage};
use crate::error::{QunMindError, Result};
use crate::source::{
    PublicNewsItem, PublicNewsSource, ScoredItem,
    build_scoring_prompt, parse_scoring_json,
};

pub struct DailyReportGenerator {
    ai: Arc<dyn AiClient>,
    news_source: Arc<dyn PublicNewsSource>,
    scoring_prompt: String,
    report_prompt: String,
}

impl DailyReportGenerator {
    pub fn new(
        ai: Arc<dyn AiClient>,
        news_source: Arc<dyn PublicNewsSource>,
        scoring_prompt: String,
        report_prompt: String,
    ) -> Self {
        Self { ai, news_source, scoring_prompt, report_prompt }
    }

    /// Pass 1: 采集 + AI 评分
    async fn score_items(&self, items: &[PublicNewsItem]) -> Result<Vec<ScoredItem>> {
        let prompt = build_scoring_prompt(items, &self.scoring_prompt);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];
        let json = self.ai.chat(&messages).await?;
        parse_scoring_json(&json, items)
    }

    /// Pass 2: 生成日报 markdown
    async fn generate_report(&self, scored: &[ScoredItem]) -> Result<String> {
        let prompt = build_daily_prompt(scored, &self.report_prompt);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];
        self.ai.chat(&messages).await
    }

    /// 完整 pipeline
    pub async fn generate(&self) -> Result<String> {
        let items = self.news_source.fetch_top_items().await?;
        if items.is_empty() {
            return Err(QunMindError::Ai("无新闻条目可生成日报".to_string()));
        }
        let scored = self.score_items(&items).await?;
        // 过滤: score >= 6，最多保留 15 条
        let mut filtered: Vec<_> = scored.into_iter()
            .filter(|s| s.ai_score >= 6.0)
            .collect();
        filtered.sort_by(|a, b| b.ai_score.partial_cmp(&a.ai_score).unwrap_or(std::cmp::Ordering::Equal));
        filtered.truncate(15);

        if filtered.is_empty() {
            return Err(QunMindError::Ai("评分后无合格新闻(>=6分)".to_string()));
        }
        self.generate_report(&filtered).await
    }
}

fn build_daily_prompt(scored: &[ScoredItem], report_prompt: &str) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut prompt = format!("{}\n\n日期: {}\n\n评分后的新闻列表:\n", report_prompt, today);
    for s in scored {
        prompt.push_str(&format!(
            "- [{}] {} (评分: {:.0}/10, 分类: {})\n  URL: {}\n  评分理由: {}\n",
            s.item.source,
            s.item.title.replace('\n', " "),
            s.ai_score,
            s.category,
            s.item.url,
            s.reason,
        ));
    }
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::QunMindError;

    struct FakeAi {
        responses: Mutex<Vec<String>>,
    }

    impl FakeAi {
        fn new(responses: Vec<String>) -> Self {
            Self { responses: Mutex::new(responses) }
        }
    }

    #[async_trait]
    impl AiClient for FakeAi {
        async fn chat(&self, _messages: &[ChatMessage]) -> Result<String> {
            let mut responses = self.responses.lock().await;
            if responses.is_empty() {
                Err(QunMindError::Ai("no responses".to_string()))
            } else {
                Ok(responses.remove(0))
            }
        }
    }

    struct FakeNewsSource {
        items: Vec<PublicNewsItem>,
    }

    #[async_trait]
    impl PublicNewsSource for FakeNewsSource {
        async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
            Ok(self.items.clone())
        }
    }

    fn test_item(title: &str) -> PublicNewsItem {
        PublicNewsItem {
            source: "test".to_string(),
            title: title.to_string(),
            url: format!("https://example.com/{}", title),
            score: None,
            comments: None,
            ai_score: None,
            category: None,
        }
    }

    fn test_scored(title: &str, score: f64, cat: &str) -> ScoredItem {
        ScoredItem {
            item: test_item(title),
            ai_score: score,
            category: cat.to_string(),
            reason: "test reason".to_string(),
        }
    }

    #[tokio::test]
    async fn generate_two_pass_pipeline() {
        let ai = Arc::new(FakeAi::new(vec![
            // Pass 1: scoring JSON
            r#"[
                {"title": "Rust news", "score": 9, "category": "Dev", "reason": "important"},
                {"title": "low quality", "score": 3, "category": "Other", "reason": "boring"}
            ]"#
            .to_string(),
            // Pass 2: report
            "日报内容".to_string(),
        ]));
        let news = Arc::new(FakeNewsSource {
            items: vec![
                test_item("Rust news"),
                test_item("low quality"),
            ],
        });

        let gen = DailyReportGenerator::new(
            ai,
            news,
            "评分prompt".to_string(),
            "日报prompt".to_string(),
        );
        let report = gen.generate().await.unwrap();
        assert_eq!(report, "日报内容");
    }

    #[tokio::test]
    async fn generate_returns_error_when_no_items() {
        let ai = Arc::new(FakeAi::new(vec![]));
        let news = Arc::new(FakeNewsSource { items: vec![] });
        let gen = DailyReportGenerator::new(
            ai, news, "scoring".to_string(), "report".to_string(),
        );
        let err = gen.generate().await.unwrap_err();
        assert!(err.to_string().contains("无新闻条目"));
    }

    #[test]
    fn build_daily_prompt_includes_scores_and_categories() {
        let scored = vec![test_scored("item1", 9.0, "AI")];
        let prompt = build_daily_prompt(&scored, "日报prompt");
        assert!(prompt.contains("日报prompt"));
        assert!(prompt.contains("item1"));
        assert!(prompt.contains("9/10"));
        assert!(prompt.contains("AI"));
    }
}
```

- [ ] **Step 3: 跑测试**

```bash
cargo test -p qunmind -- daily_report::tests
```

- [ ] **Step 4: Commit**

```bash
git add src/daily_report.rs src/lib.rs
git commit -m "feat(daily_report): add DailyReportGenerator with two-pass AI pipeline"
```

---

### Task 4: `WechatPublisher` — moonpub subprocess 封装

**Files:**
- Create: `src/wechat_publisher.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: 在 `src/lib.rs` 注册**

```rust
pub mod wechat_publisher;
```

- [ ] **Step 2: 创建 `src/wechat_publisher.rs`**

```rust
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, error};

use crate::error::{QunMindError, Result};

pub fn publish_to_wechat(markdown: &str, moonpub_bin: &str, articles_dir: &str) -> Result<String> {
    let dir = std::env::temp_dir().join("qunmind-daily");
    std::fs::create_dir_all(&dir).map_err(QunMindError::Io)?;

    let filename = format!("daily-{}.md", chrono::Utc::now().format("%Y-%m-%d"));
    let path = dir.join(&filename);
    std::fs::write(&path, markdown).map_err(QunMindError::Io)?;

    info!(path = %path.display(), bin = %moonpub_bin, "调用 moonpub 发布日报");

    let output = Command::new(moonpub_bin)
        .args(["push", &path.to_string_lossy(), "--render"])
        .env("MOONPUB_ARTICLES", articles_dir)
        .output()
        .map_err(|e| QunMindError::Channel(format!("启动 moonpub 失败: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "moonpub 发布失败");
        return Err(QunMindError::Channel(format!("moonpub 发布失败: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    info!(stdout = %stdout, "moonpub 发布成功");

    // Clean up temp file
    let _ = std::fs::remove_file(&path);

    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_errors_when_bin_not_found() {
        let result = publish_to_wechat("# test", "/nonexistent/bin/moonpub", "/tmp");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("启动 moonpub"));
    }
}
```

- [ ] **Step 3: 跑测试**

```bash
cargo test -p qunmind -- wechat_publisher::tests
```

- [ ] **Step 4: Commit**

```bash
git add src/wechat_publisher.rs src/lib.rs
git commit -m "feat(wechat_publisher): add moonpub subprocess wrapper for WeChat publishing"
```

---

### Task 5: 改造 `DailyReportScheduler` — 路由微信发布

**Files:**
- Modify: `src/scheduler/daily_report.rs`

- [ ] **Step 1: 扩展 `DailyReportTarget`**

在 `src/scheduler/daily_report.rs` 中，修改 `DailyReportTarget` struct，追加：

```rust
struct DailyReportTarget {
    chat_id: String,
    name: String,
    cron: String,
    prompt: String,
    lookback_hours: i64,
    max_messages: i64,
    max_links: i64,
    // 新增
    output: String,
    wechat_bin: String,
    wechat_articles_dir: String,
}
```

- [ ] **Step 2: 修改 `report_targets` 函数**

在 `report_targets` 函数中，`DailyReportTarget` 的构造里追加新字段的映射。找到 `config.daily_reports` 遍历构造处，追加：

```rust
output: report.output.clone(),
wechat_bin: report.wechat_bin.clone(),
wechat_articles_dir: report.wechat_articles_dir.clone(),
```

同时在 legacy 单群模式的构造中追加默认值：

```rust
output: "chat".to_string(),
wechat_bin: String::new(),
wechat_articles_dir: String::new(),
```

- [ ] **Step 3: 在 `send_report_for` 加路由逻辑**

修改 `send_report_for` 方法，在开头插入路由判断（保持现有群聊逻辑不变，仅新增 else 分支）：

```rust
async fn send_report_for(&self, target: &DailyReportTarget) {
    if target.output == "wechat" {
        self.send_report_to_wechat(target).await;
        return;
    }
    // ... 现有的群聊发送逻辑保持不变 ...
}
```

- [ ] **Step 4: 新增 `send_report_to_wechat` 方法**

在 `impl DailyReportScheduler` 中追加：

```rust
async fn send_report_to_wechat(&self, target: &DailyReportTarget) {
    use crate::daily_report::DailyReportGenerator;
    use crate::wechat_publisher::publish_to_wechat;

    let Some(source) = &self.public_news_source else {
        error!("微信日报需要启用 public_sources");
        return;
    };

    let generator = DailyReportGenerator::new(
        Arc::clone(&self.ai),
        Arc::clone(source),
        self.config.daily_report_scoring_prompt.clone(),
        target.prompt.clone(),
    );

    let markdown = match generator.generate().await {
        Ok(md) => md,
        Err(e) => {
            error!("生成微信日报失败: {}", e);
            return;
        }
    };

    match publish_to_wechat(&markdown, &target.wechat_bin, &target.wechat_articles_dir) {
        Ok(_) => info!(name = %target.name, "微信日报发布成功"),
        Err(e) => error!("微信日报发布失败: {}", e),
    }
}
```

- [ ] **Step 5: 修复测试**

测试中构造 `DailyReportTarget` 的地方都需要追加三个新字段。例如 `test_target` helper 函数：

```rust
fn test_target(chat_id: &str, prompt: &str) -> DailyReportTarget {
    DailyReportTarget {
        chat_id: chat_id.to_string(),
        name: String::new(),
        cron: "0 0 9 * * *".to_string(),
        prompt: prompt.to_string(),
        lookback_hours: 24,
        max_messages: 200,
        max_links: 20,
        output: "chat".to_string(),
        wechat_bin: String::new(),
        wechat_articles_dir: String::new(),
    }
}
```

所有 `report_targets` 测试中构造的 `DailyReportTarget` 也加同样的三个字段。

- [ ] **Step 6: 跑测试**

```bash
cargo test -p qunmind -- scheduler::daily_report::tests
```

- [ ] **Step 7: Commit**

```bash
git add src/scheduler/daily_report.rs
git commit -m "feat(scheduler): route daily report to WeChat via moonpub when output=wechat"
```

---

### Task 6: CLI — `daily-report` 子命令

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 在 `CliCommand` 加 variant**

```rust
// src/cli.rs — CliCommand enum 中追加
    /// 生成日报 markdown 并输出到文件（手动测试用）
    #[command(name = "daily-report")]
    DailyReport {
        /// 输出文件路径
        #[arg(long)]
        output: PathBuf,
        /// 回溯小时数
        #[arg(long, default_value_t = 24)]
        hours: i64,
    },
```

- [ ] **Step 2: 在 `main.rs` 处理命令**

在 `run_diagnostic_command` 中处理新的命令。找到该函数中的 `match command`：

```rust
CliCommand::DailyReport { output, hours } => {
    let ai_client = build_ai_client(&config)?;
    let public_news_source = build_public_news_source(&config)?
        .ok_or_else(|| QunMindError::Config(
            "daily-report 需要启用至少一个 public_sources".to_string()
        ))?;

    let generator = DailyReportGenerator::new(
        ai_client,
        public_news_source,
        config.schedule.daily_report_scoring_prompt.clone(),
        config.schedule.daily_report_prompt.clone(),
    );

    // daily-report CLI always generates for the last N hours
    // (the hours param controls lookback — for now, the generator
    //  fetches from the source, which returns current top items)
    let markdown = generator.generate().await?;
    std::fs::write(&output, &markdown)
        .with_context(|| format!("写入日报文件失败: {}", output.display()))?;
    println!("日报已写入 {}", output.display());
    Ok(())
}
```

注意需要在文件顶部加 `use qunmind::daily_report::DailyReportGenerator;`。

`run_diagnostic_command` 的返回值需要改成 `anyhow::Result<()>` (已经是的)。

- [ ] **Step 3: 在 `src/cli.rs` 测试中追加解析测试**

```rust
#[test]
fn parses_daily_report_command() {
    let args = parse_args(&[
        "qunmind",
        "daily-report",
        "--output",
        "/tmp/daily.md",
        "--hours",
        "48",
    ]);

    match args.command {
        Some(CliCommand::DailyReport { output, hours }) => {
            assert_eq!(output, PathBuf::from("/tmp/daily.md"));
            assert_eq!(hours, 48);
        }
        _ => panic!("daily-report command should parse"),
    }
}

#[test]
fn parses_daily_report_default_hours() {
    let args = parse_args(&[
        "qunmind",
        "daily-report",
        "--output",
        "/tmp/daily.md",
    ]);

    match args.command {
        Some(CliCommand::DailyReport { hours, .. }) => {
            assert_eq!(hours, 24);
        }
        _ => panic!("daily-report command should parse"),
    }
}
```

- [ ] **Step 4: 跑测试**

```bash
cargo test -p qunmind -- cli::tests::parses_daily_report
```

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat(cli): add daily-report subcommand for manual report generation"
```

---

### Task 7: 集成 — 全链路编译 + 端到端验证

**Files:** 无新文件

- [ ] **Step 1: 全量编译**

```bash
cargo build 2>&1
```

确保零 error、零 warning。

- [ ] **Step 2: 全量测试**

```bash
cargo test 2>&1
```

确保所有旧测试和新测试全部通过。

- [ ] **Step 3: `cargo clippy`**

```bash
cargo clippy --all-targets 2>&1
```

- [ ] **Step 4: 手动验证 CLI**

```bash
# 确认 daily-report 子命令存在
cargo run -- daily-report --help
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: ensure full build, tests, and clippy pass"
```

---

### Task 8: 更新 `config.example.toml`

**Files:**
- Modify: `config.example.toml`

- [ ] **Step 1: 追加微信日报配置示例**

在 `config.example.toml` 中找到 `[schedule]` 段，在已有 `[[schedule.daily_reports]]` 示例后追加：

```toml
# 微信公众号日报示例 (需要 moonpub 已配置)
# [[schedule.daily_reports]]
# name = "微信公众号日报"
# enabled = true
# cron = "0 0 8 * * *"
# output = "wechat"
# wechat_bin = "moonpub"
# wechat_articles_dir = "/data/moonpub"
# prompt = """你是一个科技&Web3日报编辑..."""
```

- [ ] **Step 2: Commit**

```bash
git add config.example.toml
git commit -m "docs: add WeChat daily report example to config.example.toml"
```

---

## Implementation Order

```
Task 1 (Config) → Task 2 (ScoredItem) → Task 3 (Generator) → Task 4 (Publisher)
                                                                    ↓
                                              Task 5 (Scheduler路由) ←┘
                                                                    ↓
                                              Task 6 (CLI) → Task 7 (集成) → Task 8 (doc)
```

Task 1-2 是数据基础，Task 3-4 是核心模块(可并行)，Task 5 串联，Task 6-8 收尾。
