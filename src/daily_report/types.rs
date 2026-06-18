use serde::Deserialize;

#[derive(Deserialize, Default)]
pub(crate) struct ReportJson {
    #[serde(default)]
    pub title_hint: String,
    #[serde(default)]
    pub intro: String,
    #[serde(default)]
    pub focus_text: String,
    #[serde(default)]
    pub focus_url: String,
    #[serde(default)]
    pub ai_items: Vec<ReportSection>,
    #[serde(default)]
    pub ai_signals: Vec<String>,
    #[serde(default)]
    pub web3_items: Vec<ReportSection>,
    #[serde(default)]
    pub tech_items: Vec<ReportSection>,
    #[serde(default)]
    pub tech_timeline: Vec<String>,
    #[serde(default)]
    pub reads: Vec<ReportRead>,
    #[serde(default)]
    pub summary: String,
}

#[derive(Deserialize, Default)]
pub(crate) struct ReportSection {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub points: i64,
    #[serde(default)]
    pub subsection: String,
}

#[derive(Deserialize, Default)]
pub(crate) struct ReportRead {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub summary: String,
}
