# 科技日报 + 微信公众号自动发布 — 设计文档

**日期:** 2026-06-16
**状态:** 待审批

---

## 目标

利用 qunmind 已有的多源新闻采集能力和 AI 摘要能力，生成结构化日报 markdown，自动发布到微信公众号。全部逻辑内聚在 qunmind 中，不依赖外部 shell 脚本。后续逐步将 Horizon 和 Agent-Reach 的优秀设计用 Rust 实现到 qunmind 中。

## 架构

```
qunmind (常驻进程)
     │
     ├─ BotHandler ──── 群消息处理 (已有)
     │
     └─ DailyReportScheduler (已有, 改造)
           │
           │  cron "0 0 8 * * *" 触发
           │
           ▼
        DailyReportGenerator (新增)
           │
           │  1. 多源采集 (已有 CompositePublicNewsSource)
           │  2. AI 评分过滤 (新增, 参考Horizon)
           │  3. AI 日报生成 (重写prompt)
           │  4. 写 markdown 到临时文件
           │
           ▼
        moonpub push --render (subprocess 调用)
           │
           │  markdown → WeChat HTML → draft.json → 微信草稿API
           │
           ▼
        微信公众号发布
```

**全部在 qunmind 一个进程内完成，一个 config.toml 管全部。**

## 配置

在 `DailyReportConfig` 中新增 `output` 字段：

```toml
[[schedule.daily_reports]]
name = "微信公众号日报"
enabled = true
cron = "0 0 8 * * *"
output = "wechat"                      # 新增: 输出目标 (默认 "chat")
wechat_bin = "moonpub"                 # 新增: moonpub 可执行文件路径
wechat_articles_dir = "/data/moonpub"  # 新增: moonpub 的 articles 目录
prompt = "..."                         # 自定义日报 prompt
```

- `output = "chat"`: 发到群聊 (已有行为)
- `output = "wechat"`: 生成 markdown → 调 moonpub 发布到微信公众号

## 核心模块

### 新增 `src/daily_report.rs`

```rust
pub struct DailyReportGenerator {
    ai: Arc<dyn AiClient>,
    news_source: Arc<dyn PublicNewsSource>,
    scoring_prompt: String,
    report_prompt: String,
}

impl DailyReportGenerator {
    pub async fn generate(&self, hours: i64) -> Result<String> {
        // Pass 1: 采集 + AI评分过滤
        let items = self.news_source.fetch_top_items().await?;
        let scored = self.score_items(&items).await?;
        let filtered: Vec<_> = scored.into_iter()
            .filter(|s| s.score >= 6.0)
            .take(15)
            .collect();

        // Pass 2: AI 生成日报 markdown
        let prompt = build_daily_prompt(&filtered, &self.report_prompt, hours);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];
        self.ai.chat(&messages).await
    }
}
```

### 新增 `src/wechat_publisher.rs`

封装 moonpub subprocess 调用：

```rust
pub fn publish_to_wechat(markdown: &str, bin: &str, articles_dir: &str) -> Result<()> {
    // 1. 写入临时文件
    let path = write_temp_markdown(markdown)?;
    // 2. 调 moonpub push
    let output = std::process::Command::new(bin)
        .args(["push", &path, "--render"])
        .env("MOONPUB_ARTICLES", articles_dir)
        .output()?;
    // 3. 检查结果
    if !output.status.success() { ... }
    Ok(())
}
```

### 改动范围

| 文件 | 改动 |
|------|------|
| `src/daily_report.rs` | **新文件**, 日报生成器 |
| `src/wechat_publisher.rs` | **新文件**, moonpub subprocess 封装 |
| `src/config.rs` | `DailyReportConfig` 加 `output`/`wechat_bin`/`wechat_articles_dir` 字段 |
| `src/scheduler/daily_report.rs` | 改造 `send_report_for`: 根据 `output` 类型路由到 Channel 或微信发布 |
| `src/cli.rs` | 新增 `daily-report --output <path>` 用于手动测试 |
| `src/main.rs` | 处理 daily-report CLI 命令 |

### 不做的事

- **不删** `src/scheduler/daily_report.rs` 的群聊发送逻辑，保持向后兼容
- **不改** moonpub 一行代码

## 两段式 AI Pipeline

参考 Horizon 的做法，分两段调用 AI：

**Pass 1 — 评分过滤:**
- 输入: 20-30条新闻 (title + source + url)
- AI 输出: JSON `[{"title": "...", "score": 7, "category": "AI", "reason": "..."}]`
- 保留 score >= 6 的，上限 15 条

**Pass 2 — 日报生成:**
- 输入: 过滤后的新闻 (带评分和分类)
- AI 输出: 结构化 markdown，使用 moonpub `:::` fence 语法

## 日报输出格式

```markdown
---
title: 科技 & Web3 日报 · 2026-06-16
digest: 今日共采集30条，精选8条
date: 2026-06-16
tags: [科技, Web3, AI, Crypto, 日报]
wechat_author: 寻月隐君
---

:::intro
今日共采集 30 条新闻，精选 8 条...
:::

:::callout
label: 今日必读
Solana Firedancer 客户端上线主网...
:::

## 🔥 热点速览

- **Rust 2026 Edition 正式发布** ([HN](url)) — ... (9/10)
- ...

## 📡 AI & LLM

- ...

## ⛓️ Web3 / Crypto

- ...

## 📦 开源 & 工具

- ...

:::summary
今日重点: ...
:::

---
*由 QunMind AI 生成 · 数据来源: Hacker News, CoinMarketCap, CoinGecko, DeFi Llama, GitHub Trending*
```

## 后续迭代

第一期不实现，架构已预留：

### 增强去重 (参考 Horizon)
- URL 归一化: 去掉 query params、trailing slash、www 前缀
- 标题相似度判断

### 内容富化 (参考 Horizon)
- 对高分新闻调用搜索 API 补充背景

### 新数据源

| 来源 | 参考 | 优先级 |
|------|------|--------|
| 通用 RSS/Atom | Horizon + Agent-Reach | 高 |
| Reddit | Horizon | 中 |
| 雪球 | Agent-Reach | 中 |

### 多后端路由 (参考 Agent-Reach)
- 每个 source 支持 primary + fallback
- `source` trait 加 `health_check()` 方法

## 测试策略

- `daily_report.rs`: mock AiClient + PublicNewsSource，验证两段 pipeline
- prompt 构建: 验证输出包含 frontmatter、fence 语法
- 评分 JSON 解析: 覆盖正常/格式错误
- `wechat_publisher.rs`: mock moonpub bin，验证参数传递
- `cargo test` 全量通过

## 部署

```toml
[channel]
kind = "wx_cli"

[schedule]
[[schedule.daily_reports]]
name = "微信公众号日报"
enabled = true
cron = "0 0 8 * * *"
output = "wechat"
wechat_bin = "moonpub"
wechat_articles_dir = "/data/moonpub"
```

```bash
# systemd service
qunmind --config /etc/qunmind/config.toml
```

一个进程，一个配置，跑起来后 bot+scheduler+微信发布全在。
