use crate::config::Config;
use crate::llm::{summarize, translate};
use crate::rss::fetch;
use crate::storage::{Article, FeedData};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct Scheduler {
    config: Arc<RwLock<Config>>,
}

impl Scheduler {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Scheduler { config }
    }

    pub async fn process_feed(&self, feed_name: &str, feed_url: &str) {
        tracing::info!("Processing feed: {} ({})", feed_name, feed_url);

        let raw_articles = match fetch::fetch_feed(feed_url).await {
            Ok(articles) => articles,
            Err(e) => {
                tracing::error!("Failed to fetch feed '{}': {}", feed_name, e);
                return;
            }
        };

        let mut feed_data = match FeedData::load(feed_name) {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("Failed to load feed data for '{}': {}", feed_name, e);
                return;
            }
        };

        let config = self.config.read().await.clone();
        let new_count = raw_articles.len();

        for raw in raw_articles {
            if feed_data.contains_link(&raw.link) {
                continue;
            }
            tracing::info!("New article in '{}': {}", feed_name, raw.title);

            let source_lang =
                crate::lang::detect(&raw.content).or_else(|| crate::lang::detect(&raw.title));

            let needs_trans = if raw.content.is_empty() {
                false
            } else {
                crate::lang::needs_translation(&raw.content, &config.language.target)
            };

            let (final_content, translated, detected_lang) = if needs_trans {
                match translate::translate(
                    &config.llm.translation,
                    &raw.content,
                    &config.language.target,
                )
                .await
                {
                    Ok(translated_text) => (translated_text, true, source_lang),
                    Err(e) => {
                        tracing::error!("Translation failed for '{}': {}", raw.title, e);
                        (raw.content.clone(), false, source_lang)
                    }
                }
            } else {
                (raw.content.clone(), false, source_lang)
            };

            let summary =
                match summarize::summarize(&config.llm.summary, &raw.title, &final_content).await {
                    Ok(s) => Some(s),
                    Err(e) => {
                        tracing::error!("Summarization failed for '{}': {}", raw.title, e);
                        None
                    }
                };

            let article = Article {
                id: Uuid::new_v4().to_string(),
                feed_name: feed_name.to_string(),
                title: raw.title.clone(),
                original_title: raw.title,
                link: raw.link,
                content: final_content,
                original_content: raw.content,
                summary,
                translated,
                source_lang: detected_lang,
                published_at: raw.published_at,
                processed_at: chrono::Utc::now().to_rfc3339(),
            };

            feed_data.articles.push(article);
        }

        if let Err(e) = feed_data.save(feed_name) {
            tracing::error!("Failed to save feed data for '{}': {}", feed_name, e);
        } else {
            tracing::info!(
                "Feed '{}' processed: {} new articles, {} total",
                feed_name,
                new_count,
                feed_data.article_count()
            );
        }
    }

    pub async fn process_all(&self) {
        let config = self.config.read().await;
        for feed in &config.feeds {
            if feed.enabled {
                self.process_feed(&feed.name, &feed.url).await;
            }
        }
    }

    pub async fn run_loop(self: Arc<Self>) {
        loop {
            self.process_all().await;
            let interval = {
                let config = self.config.read().await;
                config
                    .feeds
                    .iter()
                    .filter(|f| f.enabled)
                    .map(|f| f.interval_secs)
                    .min()
                    .unwrap_or(300)
            };
            tracing::info!("Next poll in {} seconds", interval);
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    }
}
