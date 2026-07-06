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
        r#"你的角色：信息通道，不是观点作者。你只做一件事——把新闻列表中最重要的技术动态筛选、归类、转述出来。不编造任何内容，不表达个人观点，不做预测和判断。每一条输出都必须能从新闻列表中找到出处，并且必须能通过对应 URL 追溯到原始资料。

{extra_context} 

根据以下新闻列表，输出一个 JSON 对象。
只输出裸 JSON，不要解释，不要代码块，不要额外前后缀。
如果某个字段拿不准，就写空字符串或空数组，不要编造。

新闻列表：
{news_list}

JSON schema（严格按此输出）：
{{
  "title_hint": "8-18字中文短标题，点出今天最具体的主线，不要写'今日科技动态'这类空标题",
  "intro": "1-2句客观说明今天主线是什么、读者先看哪类原文",
  "focus_text": "2句话以内，只写最重要的一条：谁做了什么、读者该核对什么",
  "focus_url": "对应URL",
  "ai_items": [
    {{
      "subsection": "多Agent编排|单Agent应用|工作方式变革（三选一）",
      "title": "简短标题，尽量保留原标题，不超过50字",
      "url": "原始URL（必填，必须来自新闻列表）",
      "comment": "一句话，格式接近“发生了什么；建议核对什么”。不要空话",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "ai_signals": ["AI板块信号，15字以内", "第二个信号", "第三个信号"],
  "web3_items": [
    {{
      "title": "标题，来自新闻列表时保留原标题",
      "url": "原始URL（必填，必须来自新闻列表）",
      "comment": "一句话，格式接近“发生了什么；建议核对什么”。不要写价格涨跌",
      "source": "来源平台",
      "points": 0
    }}
  ],
  "tech_items": [
    {{
      "title": "简短标题（GitHub项目尽量写 owner/repo）",
      "url": "原始URL（必填，必须来自新闻列表）",
      "comment": "一句话，格式接近“发生了什么；建议核对什么”。聚焦工程细节",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "tech_timeline": ["近期动态: 一句客观时间线"],
  "reads": [
    {{
      "title": "文章标题",
      "url": "原始URL（必填，必须来自新闻列表）",
      "summary": "2句话以内，说明作者或团队、核心内容、打开原文应重点看什么。禁止套话"
    }}
  ],
  "summary": "1-2句客观归纳，只说今天哪些方向最活跃"
}}

核心原则：
- 你是一条信息通道，不是专栏作者。
- 所有 URL 必须来自新闻列表，宁可少写，也不能编造。
- 优先一手来源：官方博客、论文、项目公告、GitHub、X 原帖、研究社区；二手媒体可以补充，但不要压过一手来源。
- 如果只有标题和 URL，就只写标题能确认的事实，不要脑补细节。
- Web3 comment 禁止写价格、涨跌、行情、交易建议。
- 禁止使用“重磅、震撼、颠覆”等渲染词。
- 禁止出现“加密货币、数字货币、虚拟货币”，统一写“Web3”。
- 不写具体日期，只写“近期”“本周”等相对表述。"#
    )
}
