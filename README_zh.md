# QunMind

`QunMind` 是一个 Rust 实现的微信群 AI 中枢与可追溯日报引擎。它优先解决真实微信场景里的三件事：消息接入、上下文回复、以及能真正推到公众号草稿箱的日报发布链路。

如果你要改这个项目，但不确定从哪里开始，先看 [docs/README.md](./docs/README.md) 和 [docs/change-routing-index.md](./docs/change-routing-index.md)。这两份文档会先回答“要改哪条链路、先看哪些文件、最后跑什么验证”，减少每次都从头翻代码和聊天记录。

## 一眼看懂

- **它首先是什么**：一个面向真实微信场景的后端中枢，核心是“消息接入、上下文回复、可追溯日报、公众号草稿发布”。
- **它为什么不是普通日报脚本**：日报不是导出 markdown 就结束，还要继续经过 lint、发布回执和历史追踪。
- **它为什么不是普通聊天机器人 demo**：消息会落库、抽链接、读短期上下文，还能按群配置决定是否回复。
- **你应该怎么进入它**：想跑机器人就看“快速开始”，想发日报就先看“推荐路径”和“日报联调状态”。

## 为什么需要 QunMind

很多“微信群 AI”项目只覆盖其中一小段：

- 只会收消息和回消息，但缺少持久化、上下文和真实群测诊断。
- 只会抓热点或拼摘要，但来源不可追溯，也不面向真实公众号发布。
- 只会调用模型，却没有把 `日报生成 -> lint -> 发布 -> 回执 -> 历史` 做成稳定链路。

`QunMind` 想解决的是一条更完整的真实路径：

- 群消息能真正落库、抽链接、做短期上下文。
- 日报既能基于群消息生成，也能在群消息为空时回退到公开来源。
- 微信公众号日报不是“导出一份 markdown 就结束”，而是能继续推到 `moonpub` 和公众号草稿箱。
- 出问题时尽量先给结构化状态、诊断结果和恢复入口，而不是只剩一条报错字符串。

## 它怎么工作

`QunMind` 当前围绕 3 个稳定边界组织：

- `Channel`：负责从企业微信或 wx-cli 类通道收发消息。
- `AiClient`：负责对接 OpenAI 兼容接口或 Hermes / 小龙虾类 Agent 平台。
- `DailyReportScheduler`：负责按日报目标生成和发送群日报。

主链路可以概括成：

1. 从 `WeCom` 或 `wx-cli` 接入消息。
2. 保存消息、抽取链接、读取同会话最近上下文。
3. 按群级启用状态、@ 规则和 persona 决定是否调用 AI 回复。
4. 需要日报时，优先使用群消息和链接情报；为空时再回退 `public_sources`。
5. 生成 markdown 后统一经过日报 lint，再决定是本地输出、群内发送，还是推送到公众号草稿箱。

```mermaid
flowchart LR
    A["WeCom / wx-cli"] --> B["保存消息与抽取链接"]
    B --> C["读取短期上下文"]
    C --> D["群级规则过滤<br/>@ / enable / persona"]
    D --> E["AI 回复"]
    B --> F["日报素材选择<br/>群消息优先 / public_sources 回退"]
    F --> G["生成日报 Markdown"]
    G --> H["日报 lint"]
    H --> I["群内发送 / 本地输出 / 公众号草稿箱"]
```

## 核心能力

- **微信群 AI 主链路**：消息接入、保存、上下文、群级 persona、回复发送。
- **可追溯日报**：焦点、正文和深读使用加粗的 `原文入口：https://...`，资料索引保留完整 URL。
- **公众号发布链路**：本地生成微信稿、固定封面、`moonpub --render` 推草稿、保存发布回执。
- **诊断与回放**：`wx-cli doctor`、`test-plan`、`handle-once`、`report-status`、`publish-history` 等只读或半自动入口。

## 适合什么场景

`QunMind` 当前最适合 3 类场景：

- **微信群 / 企业微信群里的 AI 助手中枢**：你需要的不是一次性 demo，而是能保存消息、带上下文回复、并能继续扩展群级规则的后端骨架。
- **可追溯的公众号日报生产线**：你希望日报不是“模型生成一段话就结束”，而是能保留完整来源、过 lint、推草稿、看回执。
- **需要先诊断、再联调的真实微信实验**：你不想一上来就真发消息或真推稿，而是先通过 `doctor`、`test-plan`、`report-status` 这类入口把阻塞看清楚。

相对地，它当前**不适合**被当成：

- 一个“大而全”的多平台聊天机器人平台
- 一个内建微信抓取、登录绕过、反风控的一体化采集器
- 一个已经可承诺完全无人值守发布的公众号系统

## 常见误解

- **“它是一个多平台 bot 平台”**：不是。项目主线仍然是微信 / 微信群 AI 中枢，不是要把所有平台都塞进一个大壳子。
- **“它已经内建了微信抓取和反风控能力”**：不是。公众号文章、X、RSS 等外部内容入口优先走外部 helper 或 `PublicNewsSource`，避免把高风险抓取逻辑塞进主进程。
- **“它已经能完全自动无人值守发公众号”**：不是。当前最小可用链路已经打通，但真实发布仍强调可诊断、可回执、可人工复核。
- **“它的重点只是做日报排版”**：不是。日报是重要能力，但不能盖过消息接入、持久化、上下文和真实微信联调这些主能力。

## 支持矩阵

### 通道与模型

| 类别 | 当前支持 | 状态 |
| --- | --- | --- |
| 企业微信内部群 | WebSocket 机器人 | 可用 |
| 普通微信 / 外部群 | wx-cli 本地通道 | MVP，真实联调持续补强中 |
| 模型接口 | OpenAI 兼容 API | 可用 |
| Agent 执行层 | Hermes / 爱马仕类 HTTP 适配 | 可用 |

### 日报素材来源

| 类别 | 当前支持 | 说明 |
| --- | --- | --- |
| 群消息 | 最近保存消息 + 链接情报 | 正式日报优先来源 |
| 公开来源 | Hacker News、CoinMarketCap、CoinGecko、DeFi Llama、Dune、GitHub Trending、Slerf Blog、Reddit RSS | 群消息为空时回退 |
| 中文 Web3 媒体 | PANews RSS、吴说区块链 Atom | 作为可追溯精选来源保留 |
| 官方博客 | OpenAI / Google / Cloudflare / Rust / GitHub / ECB 等 RSS / Atom | 提升一手来源密度 |
| 手工精选 | `[[public_sources.manual_items]]` | 补入你明确想推荐读者继续阅读的原文 |
| 公众号 RSS | `wechat_rss_*`、`[[public_sources.wechat_accounts]]` | 通过上游 RSS / Atom 接入，不在主进程里硬抓微信 |

### 输出与发布

| 输出目标 | 当前支持 | 说明 |
| --- | --- | --- |
| 群内日报 | 当前消息通道发送 | 可用 |
| 本地 markdown | `daily-report --output` / MCP `report_markdown` | 可用 |
| 微信公众号草稿箱 | `moonpub --render` | 最小可用链路已真实打通 |
| 发布状态可见性 | `report-status`、`publish-history`、结构化回执 | 可用 |

## 微信日报特点

- 微信公众号日报发布前会注入固定个人号封面：栏目是 `AI · Web3 最新日报`，署名是 `寻月隐君`，当前封面使用浅色底和深色主标题，避免手机预览里白字低对比；最终仍由 `moonpub --render` 负责渲染和上传。
- 微信公众号日报标题当前固定为 `AI · Web3 最新日报｜YYYY-MM-DD`，frontmatter 同时固定 `theme: notebook`，当天主线放在 `digest`、导语和焦点模块里，方便在草稿箱中快速辨认最新一条，且不会回退到全局主题造成样式漂移。
- 公众号日报默认走确定性配方：Rust 代码完成来源排序、去重、新鲜度筛选、分区、焦点、深读、资料索引、固定结尾和 lint，不再为每次成稿调用模型消耗 token；候选必须先满足“原文可读、日期可核验、主题直接相关、具备编辑价值”，才可进入焦点或正文，合约拆分等维护通知和牵强关联的社会新闻会被排除；能从发布时间或 URL 确认且超过 4 天的材料不会进入正文，除非当天可信候选不足；模型只保留给普通群聊摘要或未来明确启用的深度改写。
- 微信公众号日报正文现在按“短导语 -> 今日焦点 -> AI / Web3 / 技术、产业与政策 / 深读编号分区 -> 资料索引 / 补充阅读池”组织，不再在正文前重复输出“今日三件事”，也不在正文后再复述“今日收束”。焦点用 `callout` 块直接给出事件、价值、用法和核对入口；焦点、正文和深读统一用加粗的“原文入口”标识完整 URL，读者一眼就能找到跳转入口；文末来源区统一用 `compact-links` 小字号资料索引，单条尽量压成“序号｜短标题｜来源/短说明｜原文完整 URL”，避免链接区篇幅压过正文。
- 日报生成现在还额外做两层质量兜底：一层是模型 JSON 坏输出修复（例如尾部多余逗号、解释性尾巴），另一层是 Rust 侧 comment / summary 清洗，主动替换“相关主题相关材料”“近期受到关注”这类空泛句式。这样即使模型发挥不稳定，正文也尽量保持可读和可追溯。
- 候选选择还会限制少数容易挤占整期内容的来源类型，焦点只接受可读的原始摘要或明确中文标题；人工精选同样服从跨日报 URL 去重，单个巨鲸盈亏等个人仓位快讯不会填充正文；资料索引始终保留完整原文 URL，只压缩 URL 前的标题和来源信息，兼顾手机阅读与追溯能力。
- 微信公众号日报现在会在 `QunMind` 生成层固定补上“继续交流”标准模板结尾：正文尾部直接写入公众号「寻月隐君」、后台回复「加群」、信息整理 / 非投资建议提醒、回到原文核对，以及点赞 / 推荐 / 在看引导；它始终是全文最后一个正文模块，不再依赖微信后台模板插入。
- 如果后续继续优化封面或视觉风格，优先参考 [docs/wechat-daily-visual-blueprint.md](docs/wechat-daily-visual-blueprint.md)。这份蓝图把“固定栏目身份、固定叙事顺序、固定视觉 DNA、图像生成放在主链路外部”收口成了项目规则，避免以后每次又从头讨论封面和排版方向。

## 快速开始

### 1. 本地跑起机器人

```bash
cp config.example.toml config.toml
$EDITOR config.toml
cargo run
```

`config.toml` 包含本地凭据，已被 `.gitignore` 忽略。

### 2. 联调公众号日报

```bash
just db-create
just report-status config.toml '微信公众号日报'
just report-markdown config.toml '微信公众号日报' '/tmp/wechat-report.md'
just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'
just report-history config.toml '微信公众号日报'
```

如果你明确只想用公开来源先快速出稿：

```bash
just report-markdown-public-only config.toml '微信公众号日报' '/tmp/wechat-report-public-only.md'
```

### 3. 先做 wx-cli 真实群测前诊断

```bash
cargo run -- wx-cli doctor
cargo run -- wx-cli test-plan --capture-file wx-output.json
cargo run -- wx-cli handle-once --input wx-output.json --message-id "m-123" --limit 1 --no-send
```

下面的章节会继续展开 Docker、wx-cli、日报来源、公众号发布和多平台边界的细节。

## 推荐路径

如果你是第一次来到这个仓库，建议不要从头到尾顺序读完，而是按目标进入：

- **我想先把机器人跑起来**：先看“快速开始”，再看“Docker 部署”。
- **我想先验证公众号日报能不能发**：先看“快速开始”里的日报联调命令，再看“日报联调状态”。
- **我想先做普通微信群 / 外部群联调**：先看“快速开始”里的 wx-cli 诊断入口，再看“wx-cli 诊断”。
- **我想扩展来源或日报质量**：先看“支持矩阵”里的来源边界，再看“公共信息日报”“投研工具地图”和 `docs/change-routing-index.md`。
- **我想改代码但不知道先看哪里**：直接从 [docs/README.md](./docs/README.md) 和 [docs/change-routing-index.md](./docs/change-routing-index.md) 开始。

## 常见问题

- **为什么 README 里反复强调“可追溯”**：因为这个项目的日报不是把模型观点堆出来，而是要求正文、焦点、深读和资料索引都能回到完整原文 URL。
- **为什么不把微信抓取、浏览器绕过、代理编排都直接做进主进程**：因为这些能力变化快、风险高、维护边界差，更适合隔离成 helper、上游 RSS / Atom 或独立运维诊断。
- **为什么一直强调 `moonpub` 边界**：因为 `QunMind` 负责生成什么、何时发、状态是否 ready；平台规则、渲染和草稿推送则应该继续留在 `moonpub` 这一层。
- **为什么文档首页要做成“入口分流”**：因为这个项目链路很多，如果没有固定入口，后续每次都会从聊天记录重新解释一遍，重复成本太高。

## 当前状态

项目处在 MVP 基础阶段，还不是完全生产可用版本。当前“最基本的微信公众号日报”已经真实打通到公众号草稿箱；下一步重点是真实 wx-cli 群聊联调，以及把 `QunMind × moonpub` 微信公众号日报链路从“能真实推草稿”继续补到“内容更稳、状态更清晰、可持续灰度试跑”的状态。

当前阶段可以概括为：

- **已经完成**：核心 Rust 骨架、消息持久化、群级配置、wx-cli 诊断/重放、MCP 接入、日报生成与发布主链路，以及公众号日报最小可用草稿推送闭环。
- **正在补强**：真实 wx-cli 样本验证、`QunMind × moonpub` 联调稳定性、多群日报配置实测、日报内容质量和文档口径统一。
- **还没完成**：真实普通微信群稳定联调、生产级部署验证、长期记忆/权限/运维能力。

粗粒度进度条：

- 产品主链路：`[########--] ~80%`
- wx-cli 诊断与重放：`[########--] ~80%`
- 日报生成与发布：`[#########-] ~92%`
- 真实微信联调验证：`[#####-----] ~50%`
- 生产可用度：`[#####-----] ~50%`

如果要对外一句话介绍当前项目位置，可以用：

> QunMind 已完成微信群 AI 中枢的后端骨架、诊断链路和日报能力，其中“公众号日报 -> 草稿箱”最小链路已经真实打通；当前正在从 MVP 功能可用阶段，推进到真实微信环境更稳定、内容质量更高、可持续演示的阶段。

## 路线图

最终目标：

- 做成一个可真实运行的微信群 AI 群智中枢，而不是只停留在 demo 或日报脚本。
- 打通“收消息 -> 保存 -> 上下文 -> AI -> 回复 -> 日报”的完整闭环。
- 保持 Rust 核心、清晰模块边界、可测试、可诊断、可持续演进。

接下来几个小目标：

1. 继续拆薄 `main.rs` 和 `src/mcp/tools.rs`，减少命令层重复。
2. 扩充已建立的脱敏 wx-cli fixture，继续验证 poll / dry-run / handle-once / send。
3. 把微信公众号日报的 `moonpub` / `public_sources` / 白名单前置依赖继续暴露，减少“定时任务跑到一半才发现缺配置或 IP 漂移”。
4. 实测多群 persona、群级 context 和 `schedule.daily_reports` 组合行为。
5. 持续同步 README / PROGRESS / AGENTS，让项目进度对内对外都清楚。

## 日报联调状态

`QunMind` 当前的微信公众号日报不是独立闭环，而是依赖本地 `moonpub`：

- 调用路径：`moonpub --articles <dir> push <temp_markdown_file> --render`
- 在真实发布前，QunMind 会把固定个人号封面 PNG 写到本次临时稿件同目录，并向临时 markdown frontmatter 注入相对 `cover:`；`moonpub --render` 继续负责按平台规则渲染正文、上传封面并推送草稿
- 当前上游状态：本地 `moonpub` 已是 Beta / early adopter ready，更适合“生成草稿并人工复核”，还不适合承诺完全无人值守发布
- `wx-cli doctor` 现在会把 `output = "wechat"` 的日报目标额外标记 `config_ready` 和 `dependency_blockers`，并进一步检查 `moonpub` 在本机是否可找到、`wechat_articles_dir` 是否真的是目录

这意味着现在可以更早看出几类典型阻塞：

- `wechat_articles_dir` 没填
- `public_sources` 一个都没启用
- 配了微信公众号日报目标，但实际还没满足生成素材和推草稿的前提

当前更可信的时间判断已经更新为：

- `2026-06-24`：第一次真实公众号草稿试发成功
- `2026-06-25`：第二次真实公众号草稿试发成功，最小可用链路确认打通
- `2026-06-25`：基于内容质量优化后的 v5 预览稿完成第三次真实公众号草稿试发
- `2026-06-26`：为个人公众号 `寻月隐君` 接入固定 `AI · Web3 最新日报` 封面
- `2026-06-27`：公众号日报正文排版升级为速览、焦点、编号分区和双层链接区，更接近可直接发布的个人号日报形态
- `2026-06-28`：公众号日报继续收敛为“今日三件事 + 三段式焦点 + 值得关注条目 + 中文总结兜底”，减少信息堆叠和英文残片观感
- `2026-06-27`：ZK 手工精选版已真实推送到公众号草稿箱，回执为 `automation_state = "ok"`、`warnings = []`
- `2026-06-25` 到 `2026-06-26`：适合做内部灰度和连续试发
- 不建议把“已经能推草稿”直接表述成“完全无人值守稳定上线”

## Docker 部署

项目现在包含 Dockerfile 和 Docker Compose，可一键启动 QunMind 与
PostgreSQL：

```bash
cp config.docker.example.toml config.toml
$EDITOR config.toml
docker compose up -d --build
docker compose logs -f qunmind
```

Compose 会把 `config.toml` 只读挂载到容器内，并用命名 volume 保存
PostgreSQL 数据。完整部署步骤、wx-cli 容器边界、Release 镜像和生产检查清单见
[DEPLOYMENT.md](DEPLOYMENT.md)。

## wx-cli 诊断

真实连接普通微信 / 外部群前，可以先只验证本地 wx-cli 收发命令：

```bash
cargo run -- wx-cli doctor
cargo run -- wx-cli test-plan --capture-file wx-output.json
cargo run -- wx-cli capture --output wx-output.json
cargo run -- wx-cli doctor --input wx-output.json
cargo run -- wx-cli test-plan --input wx-output.json
cargo run -- wx-cli test-plan --input wx-output.json --shell
cargo run -- wx-cli poll
cargo run -- wx-cli poll --input wx-output.json
cargo run -- wx-cli dry-run --limit 10
cargo run -- wx-cli dry-run --input wx-output.json --limit 10
cargo run -- wx-cli dry-run --input wx-output.json --message-id "m-123"
cargo run -- wx-cli handle-once --limit 1
cargo run -- wx-cli handle-once --input wx-output.json --limit 1
cargo run -- wx-cli handle-once --input wx-output.json --message-id "m-123" --limit 1 --no-send
cargo run -- wx-cli handle-once --input wx-output.json --message-id "m-123" --limit 1
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind 诊断消息" --dry-run
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind 诊断消息"
```

`doctor`、`test-plan`、`capture`、`poll`、`dry-run` 和 `send --dry-run` 只读取配置，不会初始化 PostgreSQL、AI 客户端或机器人主循环。`doctor` 用于真实群测前检查 wx-cli 就绪度，包括发送参数占位符、AI 配置、@ 触发安全性，以及可选捕获消息里的群聊和 message_id 信号。捕获摘要会同时列出 `reply_candidate_message_ids`、`group_reply_candidate_message_ids` 和 `formal_test_readiness`；只有一个安全群聊候选时会直接给出推荐重放 `message_id`，没有群聊回复候选或群聊回复候选不止一个时，会要求正式重放前显式指定 `--message-id`。捕获摘要还会列出已配置的群级覆盖和实际生效的日报目标群，并在配置群或启用的日报目标没有出现在捕获消息里时给 warning。`test-plan` 会输出完整正式群测命令顺序，并在生成命令里保留当前 `--config` 路径，复用同一套 readiness blockers，并标明哪些步骤只是安全预览、哪些步骤会真的发微信；加 `--shell` 时会把同一份计划渲染成 shell 脚本，非发送检查可直接执行，真实发微信步骤默认注释，必须人工确认后再取消注释。带 `--input <json-file>` 时，它还会检查捕获消息，在只有一个群聊回复候选时自动填入 message_id，并输出 `selected_message` 预览；如果没有显式配置测试 chat id，也会从选中的捕获消息推断 chat_id；同时会跳过新的 capture 步骤，避免覆盖已选中 message_id 所在的捕获文件；如果候选、测试 chat_id、唯一 message_id、群聊消息或选中消息的回复触发条件仍不明确，会把计划标为未就绪并要求显式修正，避免真实发送步骤带占位符、私聊样本或空跑。没有捕获输入时，真实重放前也需要显式传入 `--message-id`。`capture` 会执行一次 `wx_cli.poll_args`，把归一化后的可复放消息写入 JSON 文件，并直接输出 `formal_test_readiness`、`recommended_commands` 和下一步正式重放建议。`doctor`、`test-plan`、`poll`、`dry-run` 或 `handle-once` 加 `--input <json-file>` 时，会解析已捕获的 wx-cli JSON 输出文件，不会再次调用 wx-cli。`dry-run` 会按 `bot.mention_names` 输出哪些消息会触发回复，但不会保存、调用 AI 或发送。`send --dry-run` 会渲染最终 `wx_cli.send_args` 命令但不向微信发送，第一次真实诊断发送前先跑它。`handle-once --no-send` 仍会写入 PostgreSQL 并调用已配置的 AI，但会把回复捕获到 JSON 输出里，不通过 wx-cli 发到微信。普通 `handle-once` 会写入 PostgreSQL，执行群聊 @ 过滤，在命中时调用已配置的 AI，并通过 `wx_cli.send_args` 回复。捕获文件里有多条消息时，`handle-once` 现在也必须显式给出 `--message-id`，不会再按 `--limit` 默认顺序处理前几条消息；失败报告里还会直接附带 `reply_candidate_message_ids` 和 `group_reply_candidate_message_ids`，方便立刻选下一步的消息 ID；即使已经显式选中消息，如果它其实是私聊样本，或者按当前 mention 规则根本不会触发回复，也会在初始化 PostgreSQL / AI 前直接返回结构化 `ok = false` JSON，而不是等到更后面才失败。第一次真实群测建议保持 `--limit 1`，避免误刷屏。

内部实现上，CLI 和 MCP 现在已经共用 `src/wx_cli_runtime.rs` 这层 wx-cli 运行时 helper，统一消息加载、dry-run 报告和 handle-once 结构化输出；同时 `main.rs` 里的 wx-cli 子命令分发也已经收口到独立的 `src/wx_cli_commands.rs`，主入口文件会继续朝“只保留顶层路由和依赖初始化”的方向收缩。

## 公共信息日报

默认情况下，日报群在回看时间内没有消息时，机器人只发送空日报提示。需要使用公共来源作为保底素材时，在配置中开启需要的来源：

```toml
[public_sources]
hacker_news_enabled = true
coinmarketcap_enabled = true
coingecko_enabled = true
defillama_enabled = true
dune_enabled = false
github_trending_enabled = true
slerf_blog_enabled = true
hacker_news_max_items = 10
coinmarketcap_max_items = 8
coingecko_max_items = 8
defillama_max_items = 8
dune_query_ids = []
github_trending_languages = ["rust", "go", "python", "typescript"]
topic_keywords = ["rust", "web3", "ai", "llm", "agent", "zkp", "solana", "ethereum"]
```

这些来源用于补充最新编程技术、Rust、Web3、crypto、AI 和 ZKP 等前沿技术信息。CoinMarketCap 用于补充加密市场 top stories，CoinGecko 用于补充 24 小时热门搜索，DeFi Llama 用于补充协议 TVL 信号，Dune 可在配置 API key 后拉取指定 query 的结果行。生成结果会明确要求模型标明“不是群内讨论总结”，避免把公共信息误当成群聊结论。

如果你感觉“今天的日报怎么又慢了”，当前优先排查顺序已经比较明确：

1. 看是不是新增了公共来源，且它自己的 timeout 设得过大。
2. 看单个 source 内部是不是还在串行抓多页、多 feed 或多 item。
3. 看上游站点本身是否在抖动、403、429 或 EOF。
4. 不要优先怀疑排版、lint、封面或发布链路，因为它们通常不是“生成很慢”的主因。

## 多群日报

旧的 `[schedule] daily_report_chat_id` 单群配置仍然可用。需要多个群分别发日报时，使用 `[[schedule.daily_reports]]` 配置多个目标；每个目标可以单独覆盖 `cron`、`prompt`、`lookback_hours`、`max_messages` 和 `max_links`，未填写的字段继承全局 `[schedule]` 默认值。

这里要特别注意：scheduler 当前按 `chrono::Utc` 解释 cron，不按机器本地时区解释。也就是说，想要北京时间早上 `08:00` 自动发，配置应写成 `cron = "0 0 0 * * *"`；如果写 `cron = "0 0 8 * * *"`，实际会在北京时间 `16:00` 触发。

## 链接情报

QunMind 会在保存文本消息时抽取 `http://` / `https://` 链接，写入 PostgreSQL 的 `message_links` 表。日报会按 `schedule.daily_report_max_links` 纳入最近去重链接，帮助模型区分普通聊天内容和文章、工具、仓库等可追踪资源。

## 投研工具地图

`src/research/tools.rs` 维护项目投研工具目录，当前按六类组织：基础信息与行情数据、链上数据与分析、项目代码与安全、社区与社交媒体、投资与资金动向、研究报告与专家观点。后续新增 CoinGecko、DeFi Llama、Dune、区块链浏览器、审计报告或社交数据来源时，优先从这个目录选择可自动化的工具，再实现成 Rust source / connector。

如果要按学习顺序推进，而不是只看工具名单，直接参考 [docs/research-tools.md](docs/research-tools.md)。其中已经整理了基础入门路径、链上分析路径、项目尽调路径和自动化优先级。

如果后续要补公众号文章、历史文章列表或 RSS 形态的公共素材入口，可以参考 `tmwgsicp/wechat-download-api` 这类独立服务，但它更适合做上游采集或研究素材补充，不适合作为 QunMind 的群消息主通道。

更准确的推进方式是：吸收它的内容获取能力边界，再按 QunMind 当前架构把真正需要的部分实现为独立公共素材源或上游 connector，而不是把整套登录、代理和反风控实现直接搬进主机器人进程。

当前已经先落了一版更轻的接入方向：`[public_sources] wechat_rss_*`。它允许 QunMind 直接消费公众号文章 RSS / Atom 上游输出，适合对接 `wechat-download-api` 一类独立服务，但仍然保持主机器人进程不承载扫码登录、代理池和反风控复杂度。

现在还可以把公众号名字和上游 feed 显式绑定到 `[[public_sources.wechat_accounts]]`，再用名字拉文章列表：

```toml
[[public_sources.wechat_accounts]]
name = "寻月隐君"
feed_url = "http://127.0.0.1:8080/xunyue/rss.xml"
aliases = ["xunyue", "寻月"]
```

```bash
cargo run -- wechat-articles --account-name '寻月隐君' --limit 20
```

这条命令会输出结构化 JSON，包含公众号名、feed URL、文章数量，以及每篇文章的标题、作者、发布时间、摘要和完整原文 URL。MCP / Agent 侧也暴露了同语义的 `wechat_articles` tool，参数是 `account_name` 和可选 `limit`。如果只给公众号名字但没有绑定 `feed_url`，QunMind 会明确报错要求先配置上游来源；它不会在主进程里绕过微信登录态、验证码或反风控去硬抓全量历史文章。

如果要手工联调日报，不再只能“随便生成一份 markdown”。`qunmind daily-report` 现在支持 `--report-name <name>` 复用已配置日报目标的 `daily_quote` 和相关目标配置；需要继续验证发布链路时，再加 `--publish`，就能沿着该目标的 publisher 边界继续往下跑，比如 `output = "wechat"` 的公众号草稿链路。

这条手工发布链路现在也会尝试保存结构化发布回执，所以一次成功的手工推草稿会立刻反映到 `report-status` 和 `publish-history`。如果外部发布成功但回执保存失败，CLI JSON 会明确给出 `publish_receipt_saved = false` 和 `publish_receipt_save_error`，方便区分“草稿没推成”和“草稿推成了但内部状态没记住”。

为了减少临近交付时的手工操作，如果配置里只有一个 `schedule.daily_reports` 目标，`qunmind daily-report` 现在会默认复用它；只有在配置了多个日报目标时，才需要显式传 `--report-name`。

手工日报现在也会优先复用正式日报目标的素材来源：如果目标群在回看窗口内已经有保存的群消息和链接情报，就直接按该目标的 prompt 和限制生成；只有群消息为空时，才回退到 `public_sources`。这样临时手工跑一版和定时正式跑出来的结果，不会再因为素材入口不同而偏离太多。

如果某个 `[[schedule.daily_reports]]` 目标本身没有配置 `chat_id`，那么这条目标在当前语义下并不会真实读取本地群消息，而是会直接落入公开来源链路；新的 `report_source` 就是为了解决这种“口头上说优先群消息、实际配置并不支持”的可见性问题。

现在连定时 `output = "wechat"` 的公众号日报也对齐到了同一条规则：如果目标群已经有保存的群消息，scheduler 会先按群消息和链接情报生成 markdown；只有群消息为空时，才回退到 `public_sources`。也就是说，“手工演练一版”和“真正定时推草稿”已经不再维护两套不同的素材语义。

当前这层 `wechat_rss` 也补了几种常见 feed 字段兼容：Atom 的 `<author><name>`、RSS 的 `dc:creator`，以及 `pubDate` / `updated` / `published` / `dc:date` 时间字段都会尽量归一成统一的 `author` 和 UTC `published_at`，减少不同上游服务接入后摘要 prompt 字段风格漂移。

如果需求变成“给一个单独的 `mp.weixin.qq.com/s/...` 链接，尽量把正文转成 markdown 并下载图片”，推荐的接入形态和 RSS 长期订阅不同。当前更适合优先参考 [`Noisepoint/mp-weixin-to-md`](https://github.com/Noisepoint/mp-weixin-to-md) 这类更轻、更聚焦“单篇 URL -> Markdown”的外部工具；[`jackwener/wechat-article-to-markdown`](https://github.com/jackwener/wechat-article-to-markdown) 仍然适合作为更重的备选参考，尤其在更强调代码块、图片下载或更强页面提取时。无论最终选哪一个，QunMind 都应把它作为显式调用的 helper：由单独的 CLI / MCP 入口去调用外部命令、读取 markdown / 图片目录 / 元数据并返回结构化结果，而不是把浏览器抓取、反检测和登录态复杂度直接内嵌进主进程。

这条显式入口现在已经有了骨架：

```bash
cargo run -- wechat-article-url \
  --url 'https://mp.weixin.qq.com/s/xxxxxxxx'
```

同一条 helper 边界现在也补了一个只读预检入口：

```bash
cargo run -- wechat-article-url-doctor \
  --url 'https://mp.weixin.qq.com/s/xxxxxxxx'
```

使用前需要先显式配置外部 helper：

```toml
[public_sources]
wechat_article_helper_bin = "/path/to/wechat-article-helper"
wechat_article_helper_output_dir = "/tmp/qunmind-wechat-article-helper"
```

如果 helper 没配置，QunMind 会在执行前直接报配置错误，而不是假装自己能在主进程里稳定抓正文。这里刻意把配置写成通用路径，而不是把项目永久绑定到某一个抓取工具的 CLI 名称。

如果外部 helper 成功产出 markdown，QunMind 现在还会最佳努力解析其中的头部信息和正文第一段，直接在结构化 JSON 里补出 `title`、`account_name`、`published_at`、`source_url`、`summary`，并继续返回 `helper_kind`、`run_dir`、`article_dir`、`markdown_path`、`images_dir`。当前适配层已经能自动识别两类已知输出布局：`mp-weixin-to-md` 这类“单次运行目录里直接生成 markdown”的平铺布局，以及 `wechat-article-to-markdown` 这类“再套一层文章目录”的嵌套布局。这样单篇公众号链接不只是“下载到一个目录里”，还可以更自然地被后续日报素材链路复用，同时继续把浏览器抓取和反风控复杂度隔离在主进程外。

新增的 `wechat-article-url-doctor` 则刻意保持只读：它会报告 helper 是否已配置、二进制是否可找到且可执行、QunMind 会走哪种适配类型、预期工作目录和参数预览，以及输出目录父路径是否存在，但不会真正抓文章、不会改代理、也不会碰微信 UI。这条边界就是为了把“先诊断、再执行”的高风险 helper 工作流固定下来。

`wechat-article-url` 本身现在也不再只在失败时抛一条裸错误字符串。只要你已经传了 `url`，即使 helper 没配置、子进程退出非零，或者 helper 成功退出却没有产出 markdown，CLI / MCP 也会尽量返回结构化失败 JSON，至少包含：

- `failure_stage`
- `message`
- `stdout_excerpt` / `stderr_excerpt`
- `recommended_doctor_command`
- `helper_known_advice`
- `doctor`（嵌套的只读预检结果）

这样后续排障时可以先看 JSON 结果，再决定是修路径、修权限、修 helper 契约，还是先单独跑 `doctor`，不用每次都从聊天记录里重新回忆规则。

如果后面要继续往“长期资料库 / 历史文章检索 / 研究记忆层”发展，方向又和单篇 helper 不同。那一层更适合借鉴 [`khoj-ai/khoj`](https://github.com/khoj-ai/khoj) 这类项目的“资料入库 + 检索增强 + Agent 使用”分层，但不适合把整套 second brain 系统直接并进 `QunMind` 主进程；对本项目来说，更合理的做法是把它当成未来辅助检索边界，而不是新的主产品形态。

X / Twitter 这类一手信息也走同样的轻量边界：通过 `[public_sources] x_rss_*` 配置 RSSHub、Nitter 兼容源或自建 X List RSS / Atom 上游，QunMind 会把它们归一成 `X RSS` 公共素材。主进程不内嵌 X 登录、抓取、代理或反风控逻辑。

如果你想让日报不只是“快讯聚合”，而是更多吸收官网 / 官方博客 / 正式技术公告，现在也可以直接启用 `[public_sources] official_blogs_*`。这条来源边界默认面向 OpenAI、Google Blog、Cloudflare Blog、Rust Blog、GitHub Blog 这类稳定可抓取的 RSS / Atom 上游，适合补充模型发布、研究博客、基础设施公告与官方技术文章，让日报更容易产出“高层面”的内容，而不是只剩市场快讯。

如果想补充社区讨论和实用问题信号，可以启用 `[public_sources] reddit_rss_*`。它只消费 `r/rust`、`r/MachineLearning`、`r/ethdev`、`r/cryptography` 这类公开 subreddit RSS / Atom，不在主进程里集成 Reddit 登录、cookie、反爬或 API key 采集。

如果临时看到特别值得推荐大家阅读的原文，但 RSS / X 上游还没稳定抓到，可以直接加到 `[[public_sources.manual_items]]`。它适合放 OpenAI 官方文章、项目公告、论文链接、X 原帖等“人工精选”素材；这些条目会进入日报素材池，并优先出现在“推荐深读”或“完整素材链接”里。

`manual_items` 会被当成用户显式精选素材，不会因为标题没有命中 `topic_keywords` 或链接域名不在内置白名单里就被过滤掉；关键词过滤主要用于 Hacker News 这类宽泛公共 feed 降噪。

如果你现在已经有公众号 RSS / Atom 上游，最短联调路径可以直接按下面跑：

```bash
just db-create
just report-status config.toml '微信公众号日报'
just report-recover-automation config.toml '微信公众号日报'
just report-markdown config.toml '微信公众号日报' '/tmp/wechat-report.md'
just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'
just report-history config.toml '微信公众号日报'
```

如果你明确只想基于公开来源先快速出稿，现在也可以直接用显式入口：

```bash
just report-markdown-public-only config.toml '微信公众号日报' '/tmp/wechat-report-public-only.md'
```

这样不会再依赖“空 `chat_id` / 空群自动回退”的隐式行为，命令语义和实际行为会更一致。

推荐顺序是：
- 先确认 `report-status` 里的 `blockers` 和 `missing_publish_env` 已清空；只要还看到 `WECHAT_APPID` / `WECHAT_SECRET`，就先不要继续真实试发
- 如果 `report-status` 或最近回执已经出现 `automation_state = "login_required"`，先执行 `report-recover-automation`，让项目一次完成公众号后台登录态复用和浏览器自动化重试
- 再生成一版本地 markdown，确认 RSS 上游内容已经进入日报素材
- 最后再执行 `report-publish` 推到 `moonpub` 的公众号草稿箱

如果你明确要求“无痕 / 隔离浏览器”，现在 CLI 和 Justfile 的后台自动化恢复、配置和预览步骤都可以显式加一次性 profile，例如：

```bash
just report-recover-automation config.toml '微信公众号日报' 'true' 'true'
just report-preview config.toml '微信公众号日报' 'true' 'true'
```

这里第二个 `true` 表示 `temporary_profile = true`，会让 `moonpub configure` / `test-yulan` 使用隔离的一次性浏览器 profile；默认仍复用持久 profile，是为了保留公众号后台登录态、减少每次扫码。注意：当前 `moonpub push --render` 本体仍需要在 `moonpub` 仓库继续补 `--temporary-profile` 支持，否则真实推草稿后的自动配置步骤仍会复用它自己的持久 profile。

如果草稿已经推送成功，但 `report-preview` / `test-yulan` 报 Chrome websocket 启动超时，不要重新生成日报，也不要先切换无痕 profile。优先检查 `~/.config/moonpub/chrome-profile` 是否同时存在 `SingletonLock` 和 `DevToolsActivePort`，确认是否有遗留的 MoonPub 自动化 Chrome 正占用该 profile；只关闭确认无用的遗留进程后，再用原持久 profile 重试预览。这个问题属于本机浏览器进程冲突，与 OpenAPI token、IP 白名单和正文内容无关。

现在 `report-status` 也会在 CLI JSON 和 MCP 输出里直接给出 `recommended_commands`。这意味着状态页不再只告诉你“下一步大概是什么”，而是尽量直接列出你接下来最该执行的命令，减少临近交付时再靠人工把状态词翻译回 shell 命令。

如果遇到微信公众号 OpenAPI 的 `errcode=40164 invalid ip`，不要把它和日报内容生成混在一起排查。QunMind 现在提供只读本机网络诊断入口，用来查看发布目标、代理环境变量、本地代理端口和 Mihomo / Clash 控制器配置状态：

```bash
cargo run -- --config config.toml report-network-status --report-name '微信公众号日报'
```

这条命令默认不修改 Clash / Mihomo profile，也不会输出订阅 URL、节点密码或 controller secret。可选环境变量是 `QUNMIND_PUBLISH_PROXY`、`MIHOMO_CONTROLLER` 和 `MIHOMO_SECRET`；输出里只显示敏感值是否已设置，并对 URL 凭据做脱敏。设置 `QUNMIND_PUBLISH_PROXY` 后，QunMind 会把它转发为 `moonpub` 子进程的标准 HTTP(S) 代理变量，让手工和定时推送走同一个明确出口，而不依赖启动 shell 恰好带有代理变量。具体边界见 `docs/local-publisher-network-diagnostics.md`。

对 MCP / Agent 调用方，现在同一个状态结果里还会继续附带 `recommended_tool_calls`。它是 shell 命令提示的结构化版本：例如目标处于 `ready_for_first_publish` 时，Agent 已经可以直接顺着状态结果去调 `report_markdown`、`report_publish` 和 `publish_history`；最近回执仍是 `automation_state = "login_required"` 时，也可以直接去调 `report_recover_automation` 和 `publish_history`，不需要再自己从命令字符串里反推下一步。

同一套恢复动作现在也已经补到 MCP，不再只有 shell/CLI 能调。也就是说，外部 Agent / MCP 客户端现在可以直接调用 `report_status`、`report_login`、`report_configure` 和 `report_recover_automation`，并且沿用和 CLI 完全一致的日报目标选择语义：只有一个日报目标时自动复用，多个目标时必须显式给 `report_name`。这些浏览器自动化工具也支持 `temporary_profile = true`，用于显式切换到一次性隔离浏览器 profile。

现在 MCP 也已经能直接走手工日报演练链路：`report_markdown` 会按和 `qunmind daily-report` 一样的“优先群消息和链接情报、群为空时再回退 public_sources”语义生成本地 markdown；`report_publish` 则只有在显式传入 `confirm_publish = true` 时，才会继续触发真实 publisher 边界。

现在手工发布结果本身也会带下一步建议。也就是说，不管是 CLI 的 `daily-report --publish`，还是 MCP 的 `report_publish`，成功后的 JSON 都不再只停在 `published = true`，还会继续给出 `follow_up_status`，以及和 `report-status` 同语义的 `recommended_commands` / `recommended_tool_calls`，方便立刻判断下一步只是去看 `publish_history`，还是应该先走 `report_recover_automation`。

当前这条链路已经完成过至少四次真实试发验证：
- `report-status` 可到 `recently_published_with_warnings`
- `publish-history` 已能查到最近成功回执
- 最新一次 ZK 手工精选版于 `2026-06-27T11:30:40.051977+00:00` 返回 `published = true`、`publish_receipt_saved = true`、`automation_state = "ok"`、`warnings = []`
- `moonpub` 原始输出里即使带 `automation: login timeout: QR code not scanned within 120s`，也没有阻止草稿成功推入公众号草稿箱
- 现在这类成功后的自动化提示也会作为结构化 `warnings` 出现在发布回执 JSON 里，不必再手工翻 `raw_output`
- 现在回执和 `report-status` 还会把这类 warning 进一步标记成 `automation_state = "login_required"`，语义不是“完全没走自动化”，而是“草稿已推成功，但浏览器自动化没真正进入后续预览/配置步骤”

近期日报重跑也会把当前输出文件和同目录最近几份 `wechat-report-*.md` / `daily-report-*.md` 作为新鲜度上下文，提取其中所有可见 `https://...` 来源链接用于焦点、正文和深读降权，避免换了输出文件名后又重复昨天的焦点与深读材料。

日报内容质量这轮也已经补了一层 Rust 侧兜底，而不是只靠多试几次模型输出：
- AI JSON 非法或字段太空时，会自动补齐非空板块，避免退化成接近空白的草稿
- 会把明显误分的条目重新平衡回正确板块，例如 `openai/codex` 不再因为包含 `dex` 被误放进 Web3
- 会过滤或重写低信号评论，补齐深读摘要
- 空板块标题不会再渲染出来
- 条目摘要会补齐句末标点，并用 `值得关注` 引导读者先看摘要；正文观点继续用 `来源依据：...` 和加粗的 `原文入口：https://...` 独立引用行展示，避免正文里堆括号
- 总结区会过滤太短或英文片段式摘要，必要时回退成自然中文提示，避免草稿里出现 `Aave Confirms...` 这类不完整内容
- 焦点、正文观点和深读推荐都会直接显示加粗的 `原文入口：https://...`；参考来源和完整素材入口保留完整 URL。如果深读没有可靠摘要，会继续给出自然的推荐理由，避免公众号渲染后只剩可点击标题、无法复制追溯
- 如果系统拿不到正文或可靠摘要，就只展示标题、来源和完整原文 URL；不会把不可核验的信息包装成自己的观点
- 参考来源会分成两层：正文实际引用过的链接，以及更长的“完整素材链接”列表，方便读者继续追溯未写入正文的原始材料

这意味着当前最准确的状态是：

- **最小可用目标已完成**：日报能生成、能推公众号草稿、能保存回执、能查历史
- **仍需人工关注的点**：微信白名单出口 IP 可能漂移、`moonpub` 仍可能返回自动化 warning、日报内容质量还需要继续打磨
- **如果回执里看到 `automation_state = "login_required"`**：优先执行 `qunmind report-recover-automation --report-name "微信公众号日报"` 或 `just report-recover-automation config.toml '微信公众号日报'`，而不是再手动拆成多步命令或误判成“系统没有走到自动化”
- **如果 `report-login` 直接报 `oneshot canceled`**：这通常不是你的扫码问题，而是当前上游 `moonpub login` 的已知缺陷。此时不要继续重复 `report-login`，直接改用 `qunmind report-recover-automation --report-name "微信公众号日报"` 或 `qunmind report-preview --report-name "微信公众号日报" --headed` 走可扫码的浏览器自动化入口。

这里的语义要记清：
- `wechat_bin` / `wechat_articles_dir` 缺失是硬 blocker
- `WECHAT_APPID` / `WECHAT_SECRET` 缺失也是 `output = "wechat"` 真实推草稿前的硬 blocker
- `wechat_daily_report_public_sources_disabled_for_empty_group_fallback` 只是 warning，表示“如果目标群当天为空，将没有 RSS / 公共来源回退素材”
- `just db-create` 只负责在默认本地 PostgreSQL 缺库时补建 `qunmind` 数据库，表结构仍由 QunMind 启动时自动初始化
- `just report-publish` 会触发真实外部发布链路，也可能把日报素材发送到当前配置的 AI / publisher
- MCP `report_publish` 也保持同样的安全边界，必须显式 `confirm_publish = true`；如果只是想先看稿件，优先用 `report_markdown`

## 多平台发布边界

`QunMind` 后续可以支持“同一份日报分发到多个平台”，但更合适的拆分不是把所有平台发布逻辑都塞进主项目。

建议固定为：

- `QunMind`：负责生成日报、调度发布时间、检查目标是否 ready、记录发布结果
- 独立发布层 / 子项目：负责平台鉴权、平台素材渲染、媒体上传、审核轮询、失败重试

现在第一步已经落到代码里：新增了 `src/publisher.rs`，通过统一的 `publish_markdown(..., PublishTarget)` 边界承接平台发布；当前只有 `PublishTarget::WechatDraft`，继续复用本地 `moonpub` 推公众号草稿。

现在成功发布后也会返回结构化 `PublishReceipt`，包含目标、目的地、发布时间、摘要和适配器原始输出，后面补发布状态持久化、多平台对账或失败重试时可以直接复用，不需要再拆一遍接口。

下一步也已经往前走了一步：即使还没有独立发布子系统，`QunMind` 也会先把成功发布的回执记下来，后面接状态历史、失败重试或多平台对账时有稳定起点。

接下来更小但很实用的一步，就是让这些回执可以从同一套存储边界里查出来。这样即使还没有完整后台，也能先看到最近几次草稿推送记录。

面向操作者的第一个入口更适合做成独立 CLI 子命令，而不是继续塞进 `doctor`。因为发布历史本质上是运行状态查看，不是 wx-cli 联调体检。

同样的边界也延续到 MCP：发布历史应该是独立 tool，而不是混进 wx-cli 诊断工具里。这样 Agent 或外部系统调它时，语义会更清楚。

如果是临近交付、只想知道“明天到底能不能用”，那就更适合直接看 `report-status` 这类专用视图：它应该直接告诉你当前 `status`、ready / 不 ready 的 blockers、下一步该做什么，以及最近有没有成功发布记录。

现在这套状态视图已经同时接到 CLI 和 MCP：操作者可以直接跑 `qunmind report-status`，Agent / 外部系统可以调 `report_status`，两边复用同一份目标选择和 blocker 判定逻辑，避免口径漂移。

现在这个对齐范围也已经扩展到自动化恢复动作：`report_login`、`report_configure` 和 `report_recover_automation` 也能从 MCP 直接调用，并保持和 CLI 一样的 `output = "wechat"` 限制与目标解析规则。

完整说明见 [docs/multi-platform-publishing.md](docs/multi-platform-publishing.md)。后续如果要接抖音，优先走官方 API 形态；小红书默认按“手动 / 半自动 / 等官方 API”处理，而不是一开始就把高风控自动化塞进 QunMind。

## AI / Agent 学习地图

`src/research/learning.rs` 维护推荐的 LLM、API、coding agent、agent 框架、Hermes 执行层、Formal Methods 和 AI x Web3 学习资源。它已经把 AI x Web3 School 的 Learning Agent 启动 Prompt / Handbook，以及 Lean 4 中文文档、Lean4 互动关卡和 Software Foundations 这类形式化验证入口作为结构化参考纳入 Rust 代码，方便后续设计 prompt、模型供应商对接、tool calling、skills、记忆和长期执行能力，同时不把微信消息主链路变成资料清单。

现在也把 JoyAI-VL-Interaction 这类多模态 Agent 参考放进了学习目录，但当前只作为“未来视频/视觉 PoC 路线图”使用，不直接并入 `QunMind` 主运行时。详见 [docs/multimodal-roadmap.md](docs/multimodal-roadmap.md)。

## 开发命令

```bash
just fmt
just clippy
just test
```

项目测试使用 `cargo nextest`，不要用 `cargo test` 替代。

## 贡献方式

QunMind 默认采用 PR-first 工作流。每个改动先创建 `codex-<short-topic>`
分支，保持 PR 聚焦，运行 `just clippy` 和 `just test`，push 分支后再向
`main` 提 pull request。完整流程和检查清单见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## Release 自动化

推送 `v*` tag 后，GitHub Actions 会先跑格式检查、clippy 和 nextest，再自动创建
GitHub Release 并发布 GHCR 镜像：

```text
ghcr.io/qiaopengjun5162/qunmind:<tag>
ghcr.io/qiaopengjun5162/qunmind:latest
```

PostgreSQL 集成测试默认被 ignore，因为它需要一个可丢弃的测试数据库。需要真实验证 `PostgresMessageStore` 时显式运行：

```bash
QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" \
  cargo nextest run --all-features --run-ignored only postgres_store
```

测试会通过 `search_path` 创建并删除临时 schema，不会写入默认 `public` schema。

## 架构约束

能用 Rust 实现的业务逻辑，优先 Rust。能被 Web / 小程序 / App 复用的逻辑，优先 Rust WASM。TypeScript 只做平台 API、渲染、宿主能力调用和胶水层。

## 许可证

MIT
