use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub feed_name: String,
    pub title: String,
    pub original_title: String,
    pub link: String,
    pub content: String,
    pub original_content: String,
    pub summary: Option<String>,
    pub translated: bool,
    pub source_lang: Option<String>,
    pub published_at: String,
    pub processed_at: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub translated_title: bool,
    #[serde(default)]
    pub translation_model: Option<String>,
    #[serde(default)]
    pub translation_tokens: Option<u32>,
    #[serde(default)]
    pub enclosure: Option<Enclosure>,
    #[serde(skip)]
    pub published_at_rfc2822: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enclosure {
    pub url: String,
    pub content_type: Option<String>,
    pub length: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedMeta {
    pub name: String,
    pub article_ids: Vec<String>,
    #[serde(default)]
    pub last_updated: Option<String>,
}

impl FeedMeta {
    pub fn load(feed_name: &str) -> Result<Self, crate::error::AppError> {
        let path = meta_path(feed_name);
        if !path.exists() {
            return Ok(FeedMeta {
                name: feed_name.to_string(),
                article_ids: vec![],
                last_updated: None,
            });
        }
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)
            .map_err(|e| crate::error::AppError::Storage(format!("parse meta TOML: {}", e)))
    }

    pub fn save(&self, feed_name: &str) -> Result<(), crate::error::AppError> {
        let dir = feed_dir(feed_name);
        std::fs::create_dir_all(&dir)?;
        let c = toml::to_string_pretty(self)
            .map_err(|e| crate::error::AppError::Storage(format!("serialize meta: {}", e)))?;
        std::fs::write(meta_path(feed_name), c)?;
        Ok(())
    }

    // TODO(Task 4): use `published_at` for sorting articles
    pub fn add_article(&mut self, article_id: &str, _published_at: &str) {
        self.article_ids.push(article_id.to_string());
        self.last_updated = Some(chrono::Utc::now().to_rfc3339());
    }
}

impl Article {
    pub fn save_to_file(&self, feed_name: &str) -> Result<(), crate::error::AppError> {
        let path = article_path(feed_name, &self.id);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let c = toml::to_string_pretty(self)
            .map_err(|e| crate::error::AppError::Storage(format!("serialize article: {}", e)))?;
        std::fs::write(path, c)?;
        Ok(())
    }

    pub fn load_from_file(
        feed_name: &str,
        article_id: &str,
    ) -> Result<Self, crate::error::AppError> {
        let path = article_path(feed_name, article_id);
        // Fallback: try raw article_id as filename for backward compatibility with
        // data created on systems where the raw URL is a valid filename.
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                let raw_path = feed_dir(feed_name)
                    .join("articles")
                    .join(format!("{}.toml", article_id));
                std::fs::read_to_string(&raw_path)?
            }
        };
        toml::from_str(&content)
            .map_err(|e| crate::error::AppError::Storage(format!("parse article TOML: {}", e)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedData {
    pub articles: Vec<Article>,
}

impl FeedData {
    pub fn load(feed_name: &str) -> Result<Self, crate::error::AppError> {
        // New format: directory with meta.toml + per-article files
        if meta_path(feed_name).exists() {
            let meta = FeedMeta::load(feed_name)?;
            let mut articles = Vec::new();
            for id in &meta.article_ids {
                match Article::load_from_file(feed_name, id) {
                    Ok(article) => articles.push(article),
                    Err(e) => tracing::warn!(
                        "Failed to load article {} for feed {}: {}",
                        id,
                        feed_name,
                        e
                    ),
                }
            }
            for a in &mut articles {
                a.published_at_rfc2822 = chrono::DateTime::parse_from_rfc2822(&a.published_at)
                    .ok()
                    .map(|dt| dt.to_rfc2822());
            }
            return Ok(FeedData { articles });
        }

        // Legacy fallback: single TOML file
        let path = data_path(feed_name);
        if !path.exists() {
            return Ok(FeedData { articles: vec![] });
        }
        let content = std::fs::read_to_string(&path)?;
        let mut data: FeedData = toml::from_str(&content)
            .map_err(|e| crate::error::AppError::Storage(format!("parse TOML: {}", e)))?;
        for a in &mut data.articles {
            a.published_at_rfc2822 = chrono::DateTime::parse_from_rfc2822(&a.published_at)
                .ok()
                .map(|dt| dt.to_rfc2822());
        }
        Ok(data)
    }

    pub fn contains_link(&self, link: &str) -> bool {
        self.articles.iter().any(|a| a.link == link)
    }

    pub fn article_count(&self) -> usize {
        self.articles.len()
    }

    pub fn translated_count(&self) -> usize {
        self.articles.iter().filter(|a| a.translated).count()
    }

    pub fn with_summary_count(&self) -> usize {
        self.articles.iter().filter(|a| a.summary.is_some()).count()
    }
}

fn feed_dir(feed_name: &str) -> PathBuf {
    super::config::Config::data_dir().join(feed_name)
}

fn meta_path(feed_name: &str) -> PathBuf {
    feed_dir(feed_name).join("meta.toml")
}

/// Convert an article ID (typically a URL) into a safe filename by percent-encoding
/// characters that are invalid in Windows file paths.
fn sanitize_filename(id: &str) -> String {
    let mut result = String::with_capacity(id.len());
    for c in id.chars() {
        match c {
            ':' | '/' | '\\' | '?' | '*' | '<' | '>' | '|' | '"' => {
                result.push_str(&format!("%{:02X}", c as u32));
            }
            _ => result.push(c),
        }
    }
    result
}

fn article_path(feed_name: &str, article_id: &str) -> PathBuf {
    feed_dir(feed_name)
        .join("articles")
        .join(format!("{}.toml", sanitize_filename(article_id)))
}

fn data_path(feed_name: &str) -> PathBuf {
    super::config::Config::data_dir().join(format!("{}.toml", feed_name))
}

pub fn all_feed_stats() -> Result<Vec<FeedStats>, crate::error::AppError> {
    let dir = super::config::Config::data_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut stats = vec![];
    let mut seen = std::collections::HashSet::new();
    for e in std::fs::read_dir(&dir)? {
        let e = e?;
        let p = e.path();
        if p.is_dir() {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let d = FeedData::load(&name)?;
            stats.push(FeedStats {
                feed_name: name.clone(),
                article_count: d.article_count(),
                translated_count: d.translated_count(),
                with_summary_count: d.with_summary_count(),
            });
            seen.insert(name);
        } else if p.extension().is_some_and(|x| x == "toml")
            && p.file_name().is_some_and(|n| n != "token_usage.toml")
        {
            let name = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if seen.contains(&name) {
                continue;
            }
            let d = FeedData::load(&name)?;
            stats.push(FeedStats {
                feed_name: name.clone(),
                article_count: d.article_count(),
                translated_count: d.translated_count(),
                with_summary_count: d.with_summary_count(),
            });
            seen.insert(name);
        }
    }
    Ok(stats)
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedStats {
    pub feed_name: String,
    pub article_count: usize,
    pub translated_count: usize,
    pub with_summary_count: usize,
}
