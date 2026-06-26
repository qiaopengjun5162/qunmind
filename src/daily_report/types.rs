use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
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

impl ReportJson {
    pub(crate) fn referenced_urls(&self) -> std::collections::HashSet<&str> {
        let mut urls = std::collections::HashSet::new();

        if !self.focus_url.trim().is_empty() {
            urls.insert(self.focus_url.trim());
        }

        for item in &self.ai_items {
            if !item.url.trim().is_empty() {
                urls.insert(item.url.trim());
            }
        }
        for item in &self.web3_items {
            if !item.url.trim().is_empty() {
                urls.insert(item.url.trim());
            }
        }
        for item in &self.tech_items {
            if !item.url.trim().is_empty() {
                urls.insert(item.url.trim());
            }
        }
        for item in &self.reads {
            if !item.url.trim().is_empty() {
                urls.insert(item.url.trim());
            }
        }

        urls
    }
}

#[derive(Deserialize, Default, Clone)]
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

#[derive(Deserialize, Default, Clone)]
pub(crate) struct ReportRead {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub summary: String,
}
