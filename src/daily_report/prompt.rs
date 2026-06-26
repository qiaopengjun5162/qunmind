use crate::source::PublicNewsItem;

pub(super) fn build_json_prompt(items: &[PublicNewsItem]) -> String {
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

    format!(
        r#"你的角色：信息通道，不是观点作者。你只做一件事——把新闻列表中最重要的技术动态筛选、归类、转述出来。不编造任何内容，不表达个人观点，不做预测和判断。每一条输出都必须能从新闻列表中找到出处。

根据以下新闻列表，输出一个 JSON 对象（不要输出任何其他内容，不要用代码块包裹，直接输出裸 JSON）。

新闻列表：
{news_list}

JSON schema（严格按此输出）：
{{
  "title_hint": "8-18字的中文标题，必须包含至少一个具体项目名、公司名或技术关键词，例如：'RustDesk登GitHub榜首，ArXiv涌现无监督RL新论文'。避免泛泛而谈如'今日科技动态'",
  "intro": "1-2句客观陈述，包含具体数据（如项目star数、论文数量、平台名称），说明今天什么方向活跃、出现了哪些值得关注的项目或论文，不做判断",
  "focus_text": "今日最重要的一条新闻，用一句话说清楚具体事件：谁做了什么、核心亮点是什么，不加评论",
  "focus_url": "对应URL",
  "ai_items": [
    {{
      "subsection": "多Agent编排|单Agent应用|工作方式变革（三选一）",
      "title": "简短标题，保留原标题，不超过50字",
      "url": "原始URL（必填，必须来自新闻列表）",
      "comment": "一句话客观描述这条内容讲什么，不评价好坏",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "ai_signals": ["AI板块值得关注的现象15字以内，客观描述不判断", "第二个信号", "第三个信号"],
  "web3_items": [
    {{
      "title": "标题，来自新闻列表时保留原标题",
      "url": "原始URL（必填！必须来自新闻列表，绝对不能留空，绝对不能编造链接）",
      "comment": "一句话客观描述这条内容讲什么，禁止出现价格/涨跌/行情/交易内容，不评价技术好坏",
      "source": "来源平台",
      "points": 0
    }}
  ],
  "tech_items": [
    {{
      "title": "简短标题（GitHub项目写'owner/repo'格式，HN文章保留原标题，不超过50字）",
      "url": "原始URL（必填，必须来自新闻列表）",
      "comment": "一句话客观描述技术亮点，不评价好坏",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "tech_timeline": ["近期动态: 基于新闻列表中的具体项目，客观描述发生了什么"],
  "reads": [
    {{
      "title": "文章标题",
      "url": "原始URL（必填，必须来自新闻列表）",
      "summary": "必填！2-3句话（不少于30字），必须包含：①作者/团队是谁；②核心方法或结论是什么；③为什么这条内容值得关注。禁止使用'这篇材料围绕...展开'、'这篇文章讲了...'等套话"
    }}
  ],
  "summary": "基于以上内容的客观归纳，只说发生了什么、哪些方向活跃，不预测未来、不评判好坏"
}}

核心原则：
- 你是一条信息通道，不是专栏作家。你筛选、归类、转述，但不创作、不评论、不预测。
- 所有 URL 必须从新闻列表中选取，不得捏造。宁可少写一条，也不能编一条。
- web3_items 只写有真实 URL 的条目。新闻列表中包含 The Defiant、Cointelegraph、CryptoSlate 等 Web3 媒体内容时，必须优先选择 3-5 条放入 web3_items，覆盖 DeFi、稳定币、Layer2、协议升级、机构动态等方向。禁止 url 为空或编造链接。
- web3_items 的标题可以保留原标题（即使原标题含价格/市场词），但 comment 中禁止出现价格、涨跌、行情、交易内容，只客观描述事件本身。
- web3_items 禁止把非Web3新闻（如Zenoh、Iroh、Lance等）强行关联到区块链——这是捏造。
- 所有 comment / summary / intro 描述必须客观，使用"出现了""发布了""讨论了"等中性动词，避免"重磅""震撼""颠覆"等渲染词。
- 禁止出现"加密货币""数字货币""虚拟货币"，一律写"Web3"。
- 不写具体日期（几月几日），只写"近期""本周"等相对表述。"#
    )
}
