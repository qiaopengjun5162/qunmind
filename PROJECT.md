# murmur — 微信群 AI 机器人

> 项目名：murmur（低语 / 默默）
> 语言：Rust
> 日期：2026-05-29
> 状态：MVP 脚手架完成

---

## 一、项目需求

### 1.1 核心目标

开发一个基于 Rust 的微信群 AI 机器人，能加入企业微信外部群，实现：

- **双向对话**：群成员 @bot 提问，AI 自动生成回复
- **定时推送**：每天定时发送日报、提醒等消息
- **多群管理**：支持同时管理多个群，不同群可配置不同行为

### 1.2 功能清单

**P0 — MVP 已实现**

| 功能 | 描述 | 状态 |
|------|------|------|
| 企业微信连接 | 通过 WebSocket 长连接接入智能机器人 | ✅ |
| @bot 回复 | 群成员 @bot 时，调用 AI 生成回复并发送 | ✅ |
| 定时日报 | Cron 定时向指定群发送日报 | ✅ |
| AI 集成 | OpenAI 兼容 API（支持 OpenRouter/DeepSeek 等） | ✅ |
| 配置管理 | TOML 配置文件 | ✅ |

**P1 — 后续迭代**

| 功能 | 描述 |
|------|------|
| 个人微信群支持 | 通过 weixin-agent-sdk-rs 接入 iLink Bot API |
| 群消息摘要 | 每天总结群里聊了什么 |
| 关键词触发 | 群里出现特定关键词时自动通知或回复 |
| 多群独立人格 | 不同群的 bot 有不同的回复风格 |
| 对话上下文 | 保持每个群的对话记忆 |
| 流式回复 | 长回复实时流式输出 |

**P2 — 远期愿景**

| 功能 | 描述 |
|------|------|
| RAG 知识库 | 基于本地文档/笔记回答群内问题 |
| 群数据分析 | 活跃度、话题分布、热门发言 |
| 插件系统 | 允许用户自定义扩展功能 |

### 1.3 非功能需求

- **稳定性**：WebSocket 断线自动重连（SDK 内置）
- **安全**：配置文件不泄露 token，支持群白名单
- **部署**：单个二进制文件

---

## 二、架构决策

### 2.1 技术调研历程

| 方案 | 描述 | 结果 |
|------|------|------|
| wechaty-grpc + padlocal | 原方案，需要付费 $20/月 | ❌ 放弃（用户不想付费） |
| WeChatFerry (Hook) | 注入 PC 微信客户端 | ❌ 放弃（限制太多） |
| weixin-agent-sdk-rs | iLink Bot 协议，免费 | 🔄 保留作为扩展通道 |
| 企业微信智能机器人 API | 官方免费，WebSocket 长连接 | ✅ 采用 |

### 2.2 最终方案：企业微信智能机器人 + iLink Bot 扩展

**主通道：企业微信智能机器人**
- 官方支持，免费
- WebSocket 长连接，无需公网 IP
- 支持主动推送消息（日报）
- 使用 `wecom-aibot-rust-sdk` crate

**扩展通道（P1）：weixin-agent-sdk-rs**
- 个人微信群支持
- iLink Bot 协议，免费
- 纯 Rust 实现

### 2.3 架构图

```
┌─────────────────────────────────────────────────────────┐
│                      murmur (Rust)                       │
│                                                         │
│  ┌───────────────┐    ┌──────────────┐    ┌──────────┐  │
│  │  Channel      │    │  Bot         │    │  AI      │  │
│  │  (trait)      │───→│  Handler     │───→│  Client  │  │
│  └───────┬───────┘    └──────────────┘    └────┬─────┘  │
│          │                                      │        │
│  ┌───────┴───────┐                    ┌────────┴──────┐ │
│  │ WeCom Channel │                    │  OpenAI API   │ │
│  │ (WebSocket)   │                    │  (兼容接口)    │ │
│  └───────┬───────┘                    └───────────────┘ │
│          │                                               │
│  ┌───────┴───────┐    ┌──────────────┐                  │
│  │ Weixin Channel│    │  Scheduler   │                  │
│  │ (iLink Bot)   │    │  (Cron)      │                  │
│  │ [P1]          │    │  定时日报     │                  │
│  └───────────────┘    └──────────────┘                  │
└─────────────────────────────────────────────────────────┘
         ↑ WebSocket                         ↑ HTTP
         │                                  │
┌────────┴────────┐              ┌───────────┴───────────┐
│  企业微信        │              │  LLM Provider         │
│  智能机器人      │              │  (OpenRouter/DeepSeek) │
└─────────────────┘              └───────────────────────┘
```

---

## 三、技术选型

| 组件 | 选型 | 说明 |
|------|------|------|
| 异步运行时 | tokio | Rust 生态标准 |
| 企业微信 SDK | wecom-aibot-rust-sdk 1.0 | WebSocket 长连接封装 |
| HTTP 客户端 | reqwest | 调用 LLM API |
| 定时任务 | cron + chrono | Cron 表达式解析 |
| 配置 | toml + serde | TOML 配置文件 |
| 日志 | tracing + tracing-subscriber | 结构化日志 |
| 错误处理 | thiserror + anyhow | 类型化错误 |
| CLI 参数 | clap | 命令行参数解析 |

---

## 四、项目结构

```
murmur/
├── Cargo.toml
├── config.example.toml        # 配置示例
├── PROJECT.md
├── src/
│   ├── main.rs                # 入口，tokio main
│   ├── config.rs              # TOML 配置加载
│   ├── error.rs               # 自定义错误类型
│   ├── channel/
│   │   ├── mod.rs             # Channel trait + MessageHandler trait
│   │   ├── wecom.rs           # 企业微信通道（WebSocket 长连接）
│   │   └── weixin.rs          # 个人微信通道（P1，iLink Bot）
│   ├── bot/
│   │   ├── mod.rs
│   │   └── handler.rs         # 消息路由：收到消息 → 调 AI → 回复
│   ├── ai/
│   │   ├── mod.rs             # AiClient trait
│   │   └── openai.rs          # OpenAI 兼容 API
│   └── scheduler/
│       ├── mod.rs
│       └── daily_report.rs    # Cron 定时日报
└── tests/
    └── integration_test.rs
```

---

## 五、配置说明

```toml
[wecom]
bot_id = "your-bot-id"       # 企业微信后台获取
secret = "your-secret"        # 企业微信后台获取

[ai]
api_url = "https://api.openai.com/v1/chat/completions"
api_key = "your-api-key"
model = "gpt-4o-mini"
system_prompt = "你是一个有用的助手，用中文回复。"

[schedule]
daily_report_cron = "0 0 9 * * *"  # 每天早上 9:00
daily_report_chat_id = ""           # 目标群 chat_id
daily_report_prompt = "请根据今天的对话生成一份简洁的日报摘要。"

[[groups]]
chat_id = "group-chat-id-1"
name = "技术群"
enabled = true
```

---

## 六、使用步骤

### 6.1 注册企业微信

1. 访问 work.weixin.qq.com 注册企业微信（免费，个人即可）
2. 登录管理后台

### 6.2 创建智能机器人

1. 管理后台 → 应用管理 → 智能机器人
2. 创建机器人，获取 bot_id 和 secret
3. 开启 API 模式，选择"长连接"

### 6.3 配置并运行

```bash
# 复制配置文件
cp config.example.toml config.toml

# 编辑配置，填入 bot_id、secret、api_key
vim config.toml

# 编译运行
cargo run
```

### 6.4 测试

1. 在企业微信群里 @bot 发消息
2. 观察终端日志，确认收到消息并回复
3. 等待定时任务触发，确认日报发送

---

## 七、成本估算

| 项目 | 费用 |
|------|------|
| 企业微信智能机器人 | 免费 |
| LLM API | $5-50/月（取决于用量） |
| 服务器（可选） | $0-20/月 |
| **合计** | **$5-70/月** |

---

## 八、开发计划

### Phase 0：可行性验证 ✅
- [x] 编译验证
- [x] wecom-aibot-rust-sdk 集成验证
- [x] 基本消息收发验证

### Phase 1：MVP ✅
- [x] 项目结构搭建
- [x] 企业微信通道实现
- [x] AI 集成（OpenAI 兼容 API）
- [x] 消息处理（@bot 回复）
- [x] 定时任务（日报）

### Phase 2：完善
- [ ] 对话上下文（保持每个群的对话记忆）
- [ ] 群白名单
- [ ] 错误处理与重连优化
- [ ] 多群独立配置

### Phase 3：扩展
- [ ] 个人微信群支持（weixin-agent-sdk-rs）
- [ ] 群消息摘要
- [ ] 关键词触发
- [ ] 流式回复

---

*文档版本：v0.3 | 最后更新：2026-05-29*
