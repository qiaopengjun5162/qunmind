# QunMind — 微信群 AI 群智中枢

> 项目名：QunMind（群智）
> 语言：Rust
> 日期：2026-05-29
> 状态：MVP 脚手架完成，当前为 Rust 微信群 AI 机器人后端服务

---

## 一、项目需求

### 1.1 核心目标

开发一个基于 Rust 的微信群 AI 机器人。当前官方可验证入口是企业微信内部群；企业微信外部群和普通微信群不能直接依赖企业微信智能机器人覆盖。

- **双向对话**：群成员 @bot 提问，AI 自动生成回复
- **定时推送**：每天定时发送日报、提醒等消息
- **多群管理**：支持同时管理多个群，不同群可配置不同行为

### 1.2 功能清单

**P0 — MVP 已实现**

| 功能 | 描述 | 状态 |
|------|------|------|
| 企业微信内部群连接 | 通过 WebSocket 长连接接入智能机器人 | ✅ |
| @bot 回复 | 群成员 @bot 时，调用 AI 生成回复并发送 | ✅ |
| 定时日报 | Cron 定时向指定群发送日报 | ✅ |
| AI 集成 | OpenAI 兼容 API（支持 OpenRouter/DeepSeek 等） | ✅ |
| Hermes 集成 | 通过 HTTP 对接爱马仕/小龙虾类 Agent 平台 | ✅ |
| wx-cli 通道 | 轮询本地 wx-cli JSON 消息并用命令模板发送回复 | ✅ |
| 群聊 @ 触发 | 可配置 `bot.mention_names`，避免机器人回复所有群消息 | ✅ |
| 消息持久化 | PostgreSQL 保存入站消息，为日报、上下文和统计提供事实表 | ✅ |
| 真实日报 | 基于最近已保存群消息生成日报 | ✅ |
| 配置管理 | TOML 配置文件 | ✅ |

**P1 — 后续迭代**

| 功能 | 描述 |
|------|------|
| 个人微信群支持 | 通过 weixin-agent-sdk-rs 接入 iLink Bot API |
| 企业微信外部群方案评估 | 单独评估客户群 API、会话内容存档或客户端采集 |
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

### 1.4 当前项目定位与可行性结论

当前项目不是聊天记录看板，而是一个常驻运行的 Rust 群智后端服务：

- 启动后读取 TOML 配置，连接企业微信智能机器人 WebSocket 长连接
- 收到文本消息后交给 OpenAI 兼容接口或 Hermes 生成回复，再通过当前通道发回群里
- 在 mention 过滤前保存入站消息，用 Cron 定时读取最近消息并向指定群发送 AI 生成的日报文本
- 通过 `Channel` / `AiClient` trait 为后续多通道、多模型留下边界

可行性判断：

- **企业微信内部群 AI 机器人：可行**。当前架构已经覆盖连接、收消息、调模型、发消息和定时任务，下一步重点是用真实企业微信 bot_id/secret 做端到端验证，并补齐测试。
- **wx-cli + Hermes 群 AI 机器人：可作为当前普通微信群/外部群 MVP 路线**。QunMind 负责通道抽象、消息处理、Hermes 调用和定时日报；本地微信消息读取和发送先由 wx-cli 兼容命令承接。
- **企业微信外部群 AI Agent：智能机器人路线不可行**。企业微信智能机器人不应再被视为外部群入口；外部群需要单独评估客户群/客户联系 API、会话内容存档、企微客户端采集或第三方托管方案。
- **外部群/普通微信群深度 AI 回复：当前现实入口仍是客户端侧或托管账号侧采集**。官方 API 可以覆盖企业成员消息、客户联系、微信客服、公众号事件等场景，但不能直接提供普通微信群或企业微信外部群的实时群聊消息流与个人身份自动回复能力。
- **wechat-radar 式群情报看板：可行，但不是当前形态**。已有 PostgreSQL 消息事实表；后续需要补会话索引、统计/摘要任务，再决定是否内置 Rust Web API 和 Rust WebAssembly 前端。
- **个人微信本地数据分析：有条件可行**。可以参考 `wx-cli` 读取本机微信会话、历史、搜索、统计和群成员数据，但它更适合作为本地数据导入/分析通道，不等价于可主动发消息的机器人通道。
- **个人微信主动聊天机器人：需谨慎拆分目标**。读取历史可参考 `wx-cli`，主动收发个人微信消息仍应独立评估 iLink Bot、WeChatFerry 或其他通道的稳定性、合规性和维护成本。

---

## 二、架构决策

### 2.1 技术调研历程

| 方案 | 描述 | 结果 |
|------|------|------|
| wechaty-grpc + padlocal | 原方案，需要付费 $20/月 | ❌ 放弃（用户不想付费） |
| WeChatFerry (Hook) | 注入 PC 微信客户端 | ❌ 放弃（限制太多） |
| weixin-agent-sdk-rs | iLink Bot 协议，免费 | 🔄 保留作为扩展通道 |
| 企业微信智能机器人 API | 官方免费，WebSocket 长连接；仅作为企业微信内部群通道 | ✅ 采用，但不能覆盖外部群 |
| winpro | 历史 Python/Django 连接器平台，集成企业微信第三方、微信公众号、微信客服、客户联系、飞书、钉钉等官方 API | 🔄 作为连接器平台、触发器/执行动作、授权回调、Token 管理和多应用适配架构参考 |
| wechat-radar | 本地优先的微信群情报看板 | 🔄 作为群摘要、链接情报、话题雷达、日报体验参考 |
| wx-cli | 本地微信数据 CLI/daemon | ✅ 已加入本地通道 MVP，继续作为数据读取、导入和发送命令形态参考 |
| Hermes / 小龙虾 / 爱马仕 | Agent 平台 | ✅ 已加入 AI provider MVP，后续按真实平台协议收紧请求和响应结构 |

### 2.2 当前方案：企业微信内部群智能机器人 + 多入口扩展

**通道 1：企业微信智能机器人**
- 官方支持，免费
- WebSocket 长连接，无需公网 IP
- 支持企业微信内部群主动推送消息（日报）
- 不支持作为企业微信外部群/客户群入口
- 使用 `wecom-aibot-rust-sdk` crate

**外部群和普通微信群扩展方向（P1/P2）**
- 企业微信外部群：单独评估客户群/客户联系 API、会话内容存档和企微客户端采集的可行边界
- 普通微信群：已提供 `wx-cli` 本地通道 MVP，用轮询命令读取消息、发送命令模板回写消息
- 主动回复：当前可通过 wx-cli 兼容命令模板承接；后续若改成 Rust 原生采集层，需要单独评估稳定性、合规性和维护成本

**通道 2：wx-cli 本地微信通道**
- 通过 `wx_cli.bin + wx_cli.poll_args` 读取 JSON 消息
- 支持 `messages`、`msg_list`、`results`、`items`、`data` 等常见包裹字段
- 通过 `wx_cli.send_args` 中的 `{chat_id}`、`{text}` 模板调用本地命令发送消息
- 适合先验证“普通微信号作为 AI Agent 账号”的群回复和日报能力

**AI Provider 2：Hermes**
- 通过 `[hermes]` 配置 HTTP API
- 请求体包含 `agent_id` 和 `messages`
- 响应兼容 `text`、`reply`、`message`、`content` 以及 `data` 包裹结构
- 真实平台协议明确后，应把兼容解析收紧成平台专用 adapter

**通用连接器平台参考**
- `winpro` 证明官方 API 连接器可以做成统一平台：授权、Token 刷新、回调验签、触发器、执行动作、字段映射、样本数据和工作流调度都应抽象成通用能力
- 新项目应避免把微信采集层、AI 调度层和内容/知识中枢耦合在一起

**实现语言约束**
- 最终项目使用 Rust 实现，参考项目只参考产品、架构和接口边界，不继承 Python/TypeScript/Next.js 技术栈
- 能用 Rust 实现的业务逻辑，优先放在 Rust crate 中
- 能被 Web / 小程序 / App 复用的逻辑，优先设计成 Rust WASM 可导出的核心模块
- TypeScript 只承担平台 API、渲染、宿主能力调用和胶水层，不承载可复用业务规则
- 校验、状态机、推荐策略、颜色/音效映射、隐私规则等跨端一致性逻辑必须集中在 Rust / Rust WASM 层，避免三端各写一套后长期分叉
- 如果需要前端，优先使用 Rust WebAssembly 技术栈实现；前端只作为控制台/看板/配置界面，不改变核心 Rust 后端边界

### 2.3 架构图

```
┌─────────────────────────────────────────────────────────┐
│                     QunMind (Rust)                       │
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
│  ┌───────┴───────┐    ┌──────────────┐    ┌───────────┐ │
│  │ WxCli Channel │    │  Scheduler   │    │  Hermes   │ │
│  │ (local CLI)   │    │  (Cron)      │    │  Adapter  │ │
│  └───────────────┘    └──────────────┘    └───────────┘ │
└─────────────────────────────────────────────────────────┘
         ↑ WebSocket                         ↑ HTTP
         │                                  │
┌────────┴────────┐              ┌───────────┴───────────┐
│  企业微信内部群  │              │  LLM Provider         │
│  智能机器人      │              │  (OpenRouter/DeepSeek) │
└─────────────────┘              └───────────────────────┘
```

### 2.4 重要参考项目

以下项目是后续路线的重要参考，需要长期保留在项目文档中：

| 仓库 | 参考价值 | 对 QunMind 的启发 |
|------|----------|------------------|
| https://github.com/zjp1997720/wechat-radar | 本地优先微信群聊情报看板，聚合群消息、话题、链接、@我的消息和群日报 | P2/P3 可以借鉴其“每日工作台、话题雷达、链接情报、群日报、本地 SQLite 存储”的产品形态 |
| https://github.com/jackwener/wx-cli | Rust 实现的本地微信数据 CLI/daemon，支持会话、聊天记录、搜索、联系人、群成员、统计、导出 | 未来若做个人微信数据分析，可把它作为只读数据源或导入器，而不是替代当前企业微信内部群发送通道 |
| /Volumes/Elements/jjy/winpro | Panda 参与过的历史连接器平台，包含企业微信第三方授权回调、微信公众号消息事件、微信客服同步与发送、客户联系、飞书、钉钉等官方 API 对接 | 新项目可以借鉴它的“应用连接器 + 触发器 + 执行动作 + 字段映射 + 工作流调度”平台思路，但不要继承其大单体形态 |

落地顺序建议：

1. 继续验证企业微信和 wx-cli 两条通道的真实收发链路，确认消息事实表字段足够稳定。
2. 基于保存的消息实现关键词触发、群活跃度、话题摘要和对话上下文。
3. 抽象 `Connector` / `Trigger` / `Action` / `Workflow`，把 `winpro` 里验证过的连接器平台能力用 Rust 重做得更模块化。
4. 再接入 `wx-cli` 做本机个人微信历史导入，补齐 wechat-radar 式本地情报能力。
5. 如果需要可视化，再引入 Rust Web API / Rust WebAssembly 前端看板；否则先保持单二进制服务形态。

---

## 三、技术选型

| 组件 | 选型 | 说明 |
|------|------|------|
| 异步运行时 | tokio | Rust 生态标准 |
| 企业微信 SDK | wecom-aibot-rust-sdk 1.0 | 企业微信智能机器人 WebSocket 长连接封装 |
| HTTP 客户端 | reqwest | 调用 LLM / Hermes API |
| 消息存储 | PostgreSQL + sqlx | 保存入站消息，供日报、上下文和统计复用 |
| 定时任务 | cron + chrono | Cron 表达式解析 |
| 配置 | toml + serde | TOML 配置文件 |
| 日志 | tracing + tracing-subscriber | 结构化日志 |
| 错误处理 | thiserror + anyhow | 类型化错误 |
| CLI 参数 | clap | 命令行参数解析 |
| 跨端业务内核 | Rust / Rust WASM | 校验、状态、推荐、映射和隐私规则集中实现，供 Web / 小程序 / App 复用 |
| 前端（如需要） | Rust WebAssembly + 平台胶水层 | 控制台、配置页、看板优先复用 Rust/WASM；TypeScript 只做平台 API、渲染和胶水 |

---

## 四、项目结构

```
qunmind/
├── .github/workflows/build.yml # CI：fmt + clippy + nextest
├── Cargo.toml
├── CONTRIBUTING.md
├── LICENSE
├── README.md
├── README_zh.md
├── PROGRESS.md
├── deny.toml
├── cliff.toml
├── .pre-commit-config.yaml
├── config.example.toml        # 配置示例
├── PROJECT.md
├── src/
│   ├── lib.rs                 # 库入口，供二进制和集成测试复用
│   ├── main.rs                # 入口，tokio main
│   ├── config.rs              # TOML 配置加载
│   ├── error.rs               # 自定义错误类型
│   ├── channel/
│   │   ├── mod.rs             # Channel trait + MessageHandler trait
│   │   ├── wecom.rs           # 企业微信通道（WebSocket 长连接）
│   │   └── wx_cli.rs          # wx-cli 本地微信通道
│   ├── bot/
│   │   ├── mod.rs
│   │   └── handler.rs         # 消息路由：收到消息 → 调 AI → 回复
│   ├── ai/
│   │   ├── mod.rs             # AiClient trait
│   │   ├── openai.rs          # OpenAI 兼容 API
│   │   └── hermes.rs          # Hermes / 爱马仕 Agent 平台适配
│   ├── storage/
│   │   ├── mod.rs             # MessageStore trait
│   │   └── postgres.rs        # PostgreSQL 消息存储
│   └── scheduler/
│       ├── mod.rs
│       └── daily_report.rs    # Cron 定时读取存储消息生成日报
└── tests/                     # 集成测试目录
```

---

## 五、配置说明

```toml
[channel]
kind = "wx_cli" # 可选: "wecom", "wx_cli"

[wecom]
bot_id = "your-bot-id"       # 企业微信后台获取
secret = "your-secret"        # 企业微信后台获取

[ai]
provider = "hermes"           # 可选: "open_ai", "hermes"
api_url = "https://api.openai.com/v1/chat/completions"
api_key = ""                  # open_ai 模式必填
model = "gpt-4o-mini"
system_prompt = "你是一个有用的助手，用中文回复。"

[hermes]
api_url = "http://127.0.0.1:8000/v1/chat"
api_key = "your-hermes-api-key"
agent_id = "your-hermes-agent-id"
timeout_secs = 60

[wx_cli]
bin = "wx"
poll_args = ["new-messages", "--json"]
send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
poll_interval_secs = 5
group_chat_id = ""

[bot]
mention_names = ["@AI助手"]

[schedule]
daily_report_cron = "0 0 1 * * *"  # UTC 01:00 = 北京时间 09:00
daily_report_chat_id = ""           # 目标群 chat_id
daily_report_lookback_hours = 24
daily_report_max_messages = 200
daily_report_prompt = "请根据今天的对话生成一份简洁的日报摘要。"

[storage]
database_url = "postgres://postgres:postgres@localhost:5432/qunmind"

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

# 编辑配置，按通道填入 bot_id/secret、Hermes 或 OpenAI 配置、wx-cli 命令模板
vim config.toml

# 编译运行
cargo run
```

### 6.4 测试

1. 企业微信内部群：在内部群里 @bot 发消息
2. wx-cli 通道：确认本地命令能输出 JSON 消息，并能用模板命令发送文本
3. 观察终端日志，确认收到消息、调用 AI 并回复
4. 等待定时任务触发，确认日报发送

---

## 七、成本估算

| 项目 | 费用 |
|------|------|
| 企业微信智能机器人（内部群） | 免费 |
| wx-cli 本地通道 | 取决于本地方案，QunMind 本身不额外收费 |
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
- [x] 企业微信内部群通道实现
- [x] AI 集成（OpenAI 兼容 API）
- [x] Hermes 集成（爱马仕/小龙虾类 Agent 平台）
- [x] wx-cli 本地通道 MVP
- [x] 消息处理（@bot 回复）
- [x] 定时任务（日报）
- [x] PostgreSQL 消息持久化
- [x] 基于已保存消息生成日报
- [x] 仓库工程化初始化（README、贡献指南、许可证、CI、deny、cliff、pre-commit、测试目录）

### Phase 2：完善
- [ ] 连接器/触发器/执行动作抽象（参考 winpro，但保持 Rust 模块化）
- [ ] 对话上下文（保持每个群的对话记忆）
- [ ] 群白名单
- [ ] 错误处理与重连优化
- [ ] 多群独立配置

### Phase 3：扩展
- [ ] 企业微信外部群/客户群入口可行性评估
- [ ] 个人微信群支持（weixin-agent-sdk-rs 或其他主动收发通道）
- [ ] wx-cli 本地微信历史导入（只读分析通道）
- [ ] wx-cli 主动发送命令适配不同实现
- [ ] 普通微信/企微账号 AI 代回复通道端到端实测
- [ ] 群消息摘要
- [ ] 关键词触发
- [ ] 流式回复
- [ ] wechat-radar 式本地情报看板

---

*文档版本：v0.4 | 最后更新：2026-06-06*
