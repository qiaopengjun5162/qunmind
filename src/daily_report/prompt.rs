use crate::source::PublicNewsItem;

pub(super) fn build_json_prompt(items: &[PublicNewsItem]) -> String {
    let mut news_list = String::new();
    for (i, item) in items.iter().enumerate() {
        let score_display = item
            .score
            .map(|s| format!("{s} points"))
            .unwrap_or_else(|| "—".to_string());
        news_list.push_str(&format!(
            "\n[{}] 来源:{} | 热度:{}\n标题: {}\nURL: {}\n",
            i + 1,
            item.source,
            score_display,
            item.title.replace('\n', " "),
            item.url,
        ));
    }

    format!(
        r#"你是「寻月隐君」公众号科技编辑，专注 AI 与 Web3 技术方向。

根据以下新闻列表，输出一个 JSON 对象（不要输出任何其他内容，不要用代码块包裹，直接输出裸 JSON）。

新闻列表：
{news_list}

JSON schema（严格按此输出）：
{{
  "title_hint": "6-16字的中文标题，概括今日核心主题，有观点有张力，不写具体日期，例如：'Rust霸榜GitHub，AI重塑内容产业'",
  "intro": "1-2句总括，今日最值得关注的信号及原因",
  "focus_text": "今日最重要一条新闻的核心价值，一句话",
  "focus_url": "对应URL",
  "ai_items": [
    {{
      "subsection": "多Agent编排|单Agent应用|工作方式变革（三选一）",
      "title": "简短标题（保留原标题，不超过50字）",
      "url": "原始URL",
      "comment": "一句话解读，说明价值与影响",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "ai_signals": ["AI板块最重要信号15字以内", "第二个信号", "第三个信号"],
  "web3_items": [
    {{
      "title": "标题（来自新闻列表时用原标题；补充背景知识时写该链具体名称，如'比特币协议层进展'、'以太坊 L2 生态'、'Solana 性能优化'）",
      "url": "原始URL（必须来自新闻列表；补充背景知识时留空字符串，绝对不能把非Web3新闻的URL填进来）",
      "comment": "纯技术解读，禁止出现价格/涨跌/行情/交易内容。补充背景知识时只写该链当前技术进展，不得强行关联新闻列表中的非Web3项目",
      "source": "来源平台（补充背景知识时填'背景知识'）",
      "points": 0
    }}
  ],
  "tech_items": [
    {{
      "title": "简短标题（GitHub项目写'owner/repo'格式，HN文章保留原标题，不超过50字）",
      "url": "原始URL",
      "comment": "一句话解读，说明技术亮点和应用价值",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "tech_timeline": ["时间点1: 描述", "时间点2: 描述"],
  "reads": [
    {{
      "title": "文章标题",
      "url": "原始URL",
      "summary": "必填！2-3句话（不少于20字），说明文章核心论点和读者具体收获，禁止留空"
    }}
  ],
  "summary": "今日趋势总结2-3句，有判断有预测，不是罗列新闻"
}}

填写要求：
- ai_items 必须覆盖三个子板块，每个子板块至少1条
- web3_items 至少3条：优先用新闻列表中与 Bitcoin/Ethereum/Solana 直接相关的技术内容（url填原始链接）；不足时补充这三条链各自的技术背景知识（url留空，直接写该链当前技术方向）
- web3_items 中禁止把非Web3新闻（如Zenoh、Iroh、Lance等）强行关联到Bitcoin/Ethereum/Solana，这是捏造
- web3_items 禁止出现任何价格、涨跌、行情、交易内容
- tech_items 至少5条，涵盖开源项目、编程语言、系统工具、架构设计、性能优化
- reads 选1-2篇最有深度的技术文章，summary 字段必须认真填写不少于20字，绝对不能留空
- 所有URL必须来自新闻列表，不得捏造
- 禁止出现"加密货币""数字货币""虚拟货币"，一律写"Web3"
- 不写具体日期（几月几日），只写"近期""本周"等相对表述"#
    )
}
