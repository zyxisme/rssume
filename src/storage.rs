use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String, pub feed_name: String, pub title: String, pub original_title: String,
    pub link: String, pub content: String, pub original_content: String,
    pub summary: Option<String>, pub translated: bool, pub source_lang: Option<String>,
    pub published_at: String, pub processed_at: String,
    #[serde(default)] pub author: Option<String>,
    #[serde(default)] pub categories: Vec<String>,
    #[serde(default)] pub translated_title: bool,
    #[serde(default)] pub enclosure: Option<Enclosure>,
    #[serde(skip)] pub published_at_rfc2822: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enclosure { pub url: String, pub content_type: Option<String>, pub length: Option<u64> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedData { pub articles: Vec<Article> }

impl FeedData {
    pub fn load(feed_name: &str) -> Result<Self, crate::error::AppError> {
        let path = data_path(feed_name);
        if !path.exists() { return Ok(FeedData { articles: vec![] }); }
        let content = std::fs::read_to_string(&path)?;
        let mut data: FeedData = toml::from_str(&content)
            .map_err(|e| crate::error::AppError::Storage(format!("parse TOML: {}", e)))?;
        for a in &mut data.articles {
            a.published_at_rfc2822 = chrono::DateTime::parse_from_rfc2822(&a.published_at)
                .ok().map(|dt| dt.to_rfc2822());
        }
        Ok(data)
    }
    pub fn save(&self, feed_name: &str) -> Result<(), crate::error::AppError> {
        let dir = super::config::Config::data_dir(); std::fs::create_dir_all(&dir)?;
        let c = toml::to_string_pretty(self)
            .map_err(|e| crate::error::AppError::Storage(format!("serialize: {}", e)))?;
        std::fs::write(data_path(feed_name), c)?; Ok(())
    }
    pub fn contains_link(&self, link: &str) -> bool { self.articles.iter().any(|a| a.link == link) }
    pub fn article_count(&self) -> usize { self.articles.len() }
    pub fn translated_count(&self) -> usize { self.articles.iter().filter(|a| a.translated).count() }
    pub fn with_summary_count(&self) -> usize { self.articles.iter().filter(|a| a.summary.is_some()).count() }
}

fn data_path(feed_name: &str) -> PathBuf {
    super::config::Config::data_dir().join(format!("{}.toml", feed_name))
}

pub fn all_feed_stats() -> Result<Vec<FeedStats>, crate::error::AppError> {
    let dir = super::config::Config::data_dir();
    if !dir.exists() { return Ok(vec![]); }
    let mut stats = vec![];
    for e in std::fs::read_dir(&dir)? {
        let e = e?; let p = e.path();
        if p.extension().is_some_and(|x| x == "toml") && p.file_name().is_some_and(|n| n != "token_usage.toml") {
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let d = FeedData::load(&name)?;
            stats.push(FeedStats { feed_name: name, article_count: d.article_count(),
                translated_count: d.translated_count(), with_summary_count: d.with_summary_count() });
        }
    }
    Ok(stats)
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedStats { pub feed_name: String, pub article_count: usize,
    pub translated_count: usize, pub with_summary_count: usize }
