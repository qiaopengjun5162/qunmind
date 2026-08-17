use crate::source::PublicNewsItem;

pub(super) fn build_json_prompt_with_context(
    items: &[PublicNewsItem],
    extra_context: Option<&str>,
) -> String {
    let mut news_list = String::new();
    for (i, item) in items.iter().enumerate() {
        let score_display = item
            .score
            .map(|s| format!("{s} points"))
            .unwrap_or_else(|| "—".to_string());
        let summary = item
            .summary
            .as_deref()
            .map(|value| value.replace('\n', " "))
            .unwrap_or_else(|| "—".to_string());
        let author = item.author.as_deref().unwrap_or("—");
        let published_at = item.published_at.as_deref().unwrap_or("—");
        news_list.push_str(&format!(
            "\n[{}] 来源:{} | 热度:{} | 作者:{} | 时间:{}\n标题: {}\nURL: {}\n摘要: {}\n",
            i + 1,
            item.source,
            score_display,
            author,
            published_at,
            item.title.replace('\n', " "),
            item.url,
            summary,
        ));
    }

    let extra_context = extra_context
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("额外要求：\n{value}\n\n"))
        .unwrap_or_default();

    format!(
        r#"你的角色：一位资深科技编辑，负责把新闻列表中最重要的技术动态筛选、归类、转述出来。你的职责是“精选 + 转述 + 一句事实性点评”：不编造内容，不做行情预测，不站队，但允许对每条动态给出“为什么值得关注、影响谁、下一步看什么”的事实性判断。每一条输出都必须能从新闻列表中找到出处，并且必须能通过对应 URL 追溯到原始资料。

{extra_context} 

根据以下新闻列表，输出一个 JSON 对象。
只输出裸 JSON，不要解释，不要代码块，不要额外前后缀。
如果某个字段拿不准，就写空字符串或空数组，不要编造。

选材要求（重要）：
- 从列表中精选 8-12 条最值得读的动态，宁缺毋滥，不要为了凑数把噪音也放进来。
- AI 板块 3-5 条，Web3 板块 2-4 条，技术/产业/政策板块 2-3 条。
- 优先一手来源：官方博客、论文、项目公告、GitHub、X 原帖、研究社区；二手媒体只做补充。
- 每条 title 必须是精炼的中文标题（15-25 字），概括这条动态最核心的一件事；英文标题要翻译成中文，严禁直接截断或保留英文省略号。
- 每条 comment 必须写清“谁做了什么、关键数字或事实”，2 句话以内，禁止套话。
- 每条 takeaway 是一句事实性短评，说明“为什么值得关注、影响谁、下一步看什么”，例如“这意味着国产模型在 Agent 赛道上进入第一梯队”“这将改变云厂商的推理成本结构”“建议后续关注它的开源许可和落地门槛”。短评必须基于新闻列表中的事实，不得预测价格或编造影响。

新闻列表：
{news_list}

JSON schema（严格按此输出）：
{{
  "title_hint": "8-18字中文短标题，点出今天最具体的主线，不要写'今日科技动态'这类空标题",
  "intro": "1-2句客观说明今天主线是什么、读者先看哪类原文",
  "focus_text": "2句话以内，只写最重要的一条：谁做了什么、关键事实是什么",
  "focus_url": "对应URL",
  "focus_takeaway": "一句事实性短评：这条为什么最重要、影响谁",
  "ai_items": [
    {{
      "subsection": "多Agent编排|单Agent应用|工作方式变革（三选一）",
      "title": "精炼中文标题，15-25字，概括核心事件，不得截断英文标题",
      "url": "原始URL（必填，必须来自新闻列表）",
      "comment": "谁做了什么：2句话以内，含关键数字或事实，禁止套话",
      "takeaway": "一句事实性短评：为什么值得关注、影响谁、下一步看什么",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "ai_signals": ["AI板块信号，15字以内", "第二个信号", "第三个信号"],
  "web3_items": [
    {{
      "title": "精炼中文标题，15-25字，概括核心事件，不得截断英文标题",
      "url": "原始URL（必填，必须来自新闻列表）",
      "comment": "谁做了什么：2句话以内，含关键数字或事实，禁止套话",
      "takeaway": "一句事实性短评：为什么值得关注、影响谁、下一步看什么",
      "source": "来源平台",
      "points": 0
    }}
  ],
  "tech_items": [
    {{
      "title": "精炼中文标题（GitHub项目写 owner/repo 并说明用途），15-25字",
      "url": "原始URL（必填，必须来自新闻列表）",
      "comment": "谁做了什么：2句话以内，聚焦工程细节或关键事实",
      "takeaway": "一句事实性短评：为什么值得关注、影响谁、下一步看什么",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "tech_timeline": ["近期动态: 一句客观时间线"],
  "reads": [
    {{
      "title": "精炼中文标题，概括这篇深读文章的主题",
      "url": "原始URL（必填，必须来自新闻列表）",
      "summary": "2句话以内：说明作者或团队、核心内容、为什么值得打开原文细读。禁止套话"
    }}
  ],
  "summary": "1-2句客观归纳，只说今天哪些方向最活跃"
}}

核心原则：
- 你是资深编辑，做“精选 + 转述 + 一句事实性点评”，不是信息搬运工。
- 所有 URL 必须来自新闻列表，宁可少写，也不能编造。
- takeaway 只做基于事实的影响判断（为什么重要、影响谁、下一步看什么），禁止预测价格、禁止站队、禁止煽动。
- 如果只有标题和 URL，就只写标题能确认的事实，不要脑补细节。
- 禁止使用“值得关注：”“建议核对”“建议打开原文”“适合重点核对”“实用潜力”等套话；每一条 comment 和 takeaway 都要像编辑写出来的具体说明。
- Web3 comment 禁止写价格、涨跌、行情、交易建议。
- 禁止使用“重磅、震撼、颠覆”等渲染词。
- 禁止出现“加密货币、数字货币、虚拟货币”，统一写“Web3”。
- 不写具体日期，只写“近期”“本周”等相对表述。

写法示例（参考，不要照抄内容）：
{{
  "title_hint": "AI 编程助手进入 Agent 编排阶段",
  "intro": "今天的公共素材主线集中在 AI Agent 编排与 Web3 基础设施，先看 AI 前沿的模型能力变化，再读 Web3 的链上数据与监管动态。",
  "focus_text": "OpenAI 发布新版 Codex 编排方案，强调多 Agent 协作与任务拆分，支持 1M 上下文。",
  "focus_url": "https://openai.com/index/codex",
  "focus_takeaway": "多 Agent 协作正在从演示走向工程落地，值得关注它对企业级开发流程的影响。",
  "ai_items": [
    {{
      "subsection": "多Agent编排",
      "title": "OpenAI 发布 Codex 编排方案",
      "url": "https://openai.com/index/codex",
      "comment": "OpenAI 推出新版 Codex，支持多 Agent 协作与任务拆分，上下文扩到 1M。",
      "takeaway": "这标志多 Agent 编排开始有可用的工程方案，开发者可以关注接入成本。",
      "source": "OpenAI",
      "points": 120
    }}
  ],
  "ai_signals": ["Agent 编排热度上升", "上下文窗口继续扩大"],
  "web3_items": [
    {{
      "title": "Solana 链上活跃地址创三个月新高",
      "url": "https://example.com/solana-active",
      "comment": "Solana 近一周活跃地址环比增长 20%，开发者活动集中在 DeFi 协议。",
      "takeaway": "链上活跃度回升可能带动基础设施层关注度，值得跟踪后续生态数据。",
      "source": "吴说区块链",
      "points": 90
    }}
  ],
  "tech_items": [
    {{
      "title": "rust-lang/rust 发布 1.88 版本",
      "url": "https://github.com/rust-lang/rust",
      "comment": "Rust 1.88 稳定了异步运行时改进，编译器性能提升约 5%。",
      "takeaway": "编译器性能提升会直接降低大型 Rust 项目的构建成本。",
      "source": "GitHub Trending",
      "points": 1200
    }}
  ],
  "tech_timeline": ["近期动态: Rust 发布 1.88 版本"],
  "reads": [
    {{
      "title": "为什么大模型仍然无法可靠地规划",
      "url": "https://example.com/planning",
      "summary": "作者系统梳理了 LLM 规划能力的实验证据与局限，指出评估方法本身需要改进。"
    }}
  ],
  "summary": "今天 AI Agent 编排与 Web3 链上活跃度最值得继续跟进。"
}}"#
    )
}
