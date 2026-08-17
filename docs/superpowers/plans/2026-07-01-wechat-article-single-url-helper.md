# 2026-07-01 微信公众号单链接提取 Helper 方案

## 背景

当前 `QunMind` 已经有两条与公众号文章相关的能力边界：

1. `wechat_rss_*` / `wechat_accounts`
   - 适合长期订阅、文章列表、日报素材回退。
   - 优点是稳定、可配置、容易测试。
   - 缺点是前提必须先有 RSS / Atom / API 上游。

2. `manual_items`
   - 适合“我已经有原文 URL，先把它放进日报素材池”。
   - 优点是简单直接。
   - 缺点是拿不到正文、封面、代码块、图片等结构化内容。

用户新的真实需求是第三条：

- 给一个 `https://mp.weixin.qq.com/s/...` 链接
- 最佳努力提取标题、公众号名称、发布时间、正文 markdown、图片、本地资产路径
- 作为“研究素材 / 单篇文章归档 / 推荐深读补料”的辅助能力

这条能力不应破坏当前边界，也不应把登录态、代理池或反风控逻辑直接塞进 `QunMind` 主进程。

## 参考项目

候选上游目前分成两档：

1. `Noisepoint/mp-weixin-to-md`
   - 仓库：<https://github.com/Noisepoint/mp-weixin-to-md>
   - 更轻、更聚焦“单篇 URL -> Markdown”
   - 适合当默认优先参考
   - 更符合 `QunMind` 需要的“显式 helper、失败隔离、只做单篇链接”的边界
2. `jackwener/wechat-article-to-markdown`
   - 仓库：<https://github.com/jackwener/wechat-article-to-markdown>
   - 形态：Python CLI，不是 Rust crate
   - 核心能力：
     - 单篇公众号文章抓取
     - HTML -> Markdown
     - 图片下载到本地 `images/`
     - 保留代码块语言 fence
     - 提取标题、公众号名、发布时间、原文链接
   - 风险边界：
     - 依赖 `Camoufox` 反检测抓取
     - 天然属于高波动浏览器抓取路径
     - 不适合作为主进程内建硬依赖

## 决策

采用“外部 helper，可选接入”的方式，不把抓取实现并入 `QunMind` 主进程。

同时明确两条补充原则：

- helper 契约优先于具体实现，不能把 `wechat-article-url` 永久绑定到某一个上游 CLI 的参数和目录布局
- 如果后续要同时兼容轻量 helper 与更重 helper，应先扩适配层，不要把不同抓取器的分支逻辑直接散进 CLI / MCP 命令入口

### 要做的

后续优先补一个轻量适配层，例如：

- CLI：
  - `qunmind wechat-article-url --url <mp_url>`
- MCP：
  - `wechat_article_url`
- 输出：
  - 结构化 JSON
  - 可选 markdown 文件路径
  - 可选图片目录路径

### 不做的

- 不在 `QunMind` 里直接维护微信登录态
- 不在 `QunMind` 里直接维护代理池 / 验证码 / 反风控
- 不承诺“只给公众号名字就能抓全量历史文章”
- 不把这个入口当成日报默认主路径

## 推荐接入形态

### Phase 1

只做“外部命令适配”：

- 配置项中声明 helper 可执行文件路径
- `QunMind` 只负责：
  - 校验 URL
  - 调用外部命令
  - 读取结果目录 / stdout
  - 转成结构化 JSON
- 如果 helper 不存在或失败：
  - 明确报错
  - 不影响主消息链路、RSS、日报生成

### Phase 2

把结果接到素材边界，而不是直接耦合进 scheduler：

- 可把单篇文章结果转成：
  - `manual_items` 候选
  - 临时 `PublicNewsItem`
  - 深读候选
- 但只在用户显式调用时触发

### Phase 3

如果验证稳定，再考虑：

- `--emit-public-news-item`
- `--emit-markdown`
- `--emit-summary-json`
- 与 `moonpub` / 文章归档工具链衔接

## 输出契约建议

最小结构化字段建议：

```json
{
  "ok": true,
  "url": "https://mp.weixin.qq.com/s/...",
  "title": "文章标题",
  "account_name": "公众号名",
  "author": "作者",
  "published_at": "2026-07-01T08:00:00+08:00",
  "cover_url": "https://...",
  "markdown_path": "/tmp/.../article.md",
  "images_dir": "/tmp/.../images",
  "summary": "正文前几段或提取摘要",
  "raw_output_path": "/tmp/.../output-dir"
}
```

如果拿不到正文，也应允许退化输出：

```json
{
  "ok": false,
  "url": "https://mp.weixin.qq.com/s/...",
  "error": "helper_failed_or_article_blocked"
}
```

## 为什么这样做

因为这条能力本质上是“高波动内容抓取工具”，不是“微信群 AI 中枢”的稳定核心。

对 `QunMind` 来说，更合理的职责是：

- 复用稳定上游：RSS / Atom / API
- 在必要时调用可选 helper
- 消费结构化结果
- 保持失败隔离

而不是：

- 自己承接浏览器自动化细节
- 自己承接反检测策略
- 自己变成公众号抓取平台

## 下一步

1. 在配置层增加可选 helper 路径设计
2. 在 CLI / MCP 增加单篇链接入口草案
3. 为“helper 不存在 / 返回目录结构不符合预期 / markdown 缺失”补错误语义
4. 只在显式调用时触发，不接入默认 scheduler
