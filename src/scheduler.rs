use crate::config::Config;
use crate::llm::retry::RetryContext;
use crate::monitor::{FeedStatus, Monitor};
use crate::rss::fetch;
use crate::storage::{Article, Enclosure, FeedData, FeedMeta};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;

#[derive(Clone, Copy)]
struct RetryConfig {
    max_retries: u32,
    delay_secs: u64,
}

#[derive(Clone)]
pub struct Scheduler {
    config: Arc<RwLock<Config>>,
    monitor: Arc<RwLock<Monitor>>,
    semaphore: Arc<Semaphore>,
}

impl Scheduler {
    pub fn new(config: Arc<RwLock<Config>>, monitor: Arc<RwLock<Monitor>>) -> Self {
        let max_concurrent = config
            .try_read()
            .map(|c| c.llm.max_concurrent_requests)
            .unwrap_or(3);
        Scheduler {
            config,
            monitor,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub async fn process_feed(&self, feed_name: &str, feed_url: &str) {
        tracing::info!(feed = feed_name, url = feed_url, "processing");
        let start = std::time::Instant::now();
        self.monitor.write().await.ensure_feed(feed_name);
        self.monitor
            .write()
            .await
            .set_status(feed_name, FeedStatus::Fetching);

        let raw_articles = match fetch::fetch_feed(feed_url).await {
            Ok(a) => a,
            Err(e) => {
                let ms = start.elapsed().as_millis() as u64;
                tracing::error!(feed = feed_name, "fetch failed: {}", e);
                self.monitor
                    .write()
                    .await
                    .finish_fetch(feed_name, ms, Some(&e.to_string()));
                return;
            }
        };

        tracing::info!(
            feed = feed_name,
            fetched = raw_articles.len(),
            "fetched articles"
        );

        let max_articles = {
            let cfg = self.config.read().await;
            cfg.feeds
                .iter()
                .find(|f| f.name == feed_name)
                .map(|f| f.max_articles)
                .unwrap_or(25)
        };
        let raw_articles: Vec<_> = raw_articles.into_iter().take(max_articles).collect();

        self.monitor.write().await.finish_fetch(
            feed_name,
            start.elapsed().as_millis() as u64,
            None,
        );

        let feed_data = match FeedData::load(feed_name) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(feed = feed_name, "load failed: {}", e);
                return;
            }
        };

        let config = self.config.read().await.clone();
        let new_articles: Vec<_> = raw_articles
            .into_iter()
            .filter(|a| !feed_data.contains_link(&a.link))
            .collect();

        if new_articles.is_empty() {
            tracing::info!(feed = feed_name, "no new articles");
            self.monitor
                .write()
                .await
                .set_status(feed_name, FeedStatus::Done);
            return;
        }

        tracing::info!(
            feed = feed_name,
            new = new_articles.len(),
            "new articles to process"
        );

        let mut feed_meta = match FeedMeta::load(feed_name) {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(feed = feed_name, "meta load failed: {}", e);
                return;
            }
        };

        let total = new_articles.len() as u32;

        self.monitor.write().await.set_status(
            feed_name,
            FeedStatus::Translating {
                completed: 0,
                total,
                in_progress: vec![],
            },
        );

        let tc = config.llm.translation.clone();
        let target = config.language.target.clone();
        let retry = RetryConfig {
            max_retries: config.llm.max_retries,
            delay_secs: config.llm.retry_delay_secs,
        };
        let semaphore = self.semaphore.clone();
        let monitor = self.monitor.clone();
        let feed_name_owned = feed_name.to_string();

        let handles: Vec<_> = new_articles
            .into_iter()
            .map(|raw| {
                let tc = tc.clone();
                let target = target.clone();
                let semaphore = semaphore.clone();
                let monitor = monitor.clone();
                let feed_name = feed_name_owned.clone();

                let title = raw.title.clone();
                let link = raw.link.clone();
                let published_at = raw.published_at.clone();
                let guid = raw.guid.clone();
                let raw_content = raw.content.clone();
                tokio::spawn(async move {
                    let result = process_single_article(
                        &feed_name,
                        raw,
                        &tc,
                        &target,
                        semaphore,
                        monitor.clone(),
                        retry,
                    )
                    .await;
                    (
                        feed_name,
                        title,
                        link,
                        published_at,
                        guid,
                        raw_content,
                        result,
                    )
                })
            })
            .collect();

        let mut translated_count: u32 = 0;
        let mut failed_count: u32 = 0;
        let mut total_tokens: u32 = 0;

        for handle in handles {
            match handle.await {
                Ok((feed_name, title, _link, _published_at, _guid, _raw_content, Ok(article))) => {
                    if article.translated {
                        translated_count += 1;
                    }
                    if let Some(tokens) = article.translation_tokens {
                        total_tokens += tokens;
                    }
                    if let Err(e) = article.save_to_file(&feed_name) {
                        tracing::error!(
                            feed = feed_name,
                            article = title.as_str(),
                            "save failed: {}",
                            e
                        );
                        monitor.write().await.complete_article(&feed_name, &title);
                        continue;
                    }
                    feed_meta.add_article(&article.id, &article.published_at);
                    if let Err(e) = feed_meta.save(&feed_name) {
                        tracing::error!(feed = feed_name, "meta save failed: {}", e);
                    }
                    monitor.write().await.complete_article(&feed_name, &title);
                }
                Ok((feed_name, title, link, published_at, guid, raw_content, Err(e))) => {
                    failed_count += 1;
                    tracing::error!(
                        feed = feed_name,
                        article = title.as_str(),
                        "processing failed: {}",
                        e
                    );
                    // Save raw article to avoid reprocessing
                    let article = Article {
                        id: guid.unwrap_or_else(|| Uuid::new_v4().to_string()),
                        feed_name: feed_name.clone(),
                        title: title.clone(),
                        original_title: title.clone(),
                        link,
                        content: raw_content.clone(),
                        original_content: raw_content,
                        summary: None,
                        translated: false,
                        translated_title: false,
                        source_lang: None,
                        published_at,
                        published_at_rfc2822: None,
                        processed_at: chrono::Utc::now().to_rfc3339(),
                        author: None,
                        categories: vec![],
                        translation_model: None,
                        translation_tokens: None,
                        enclosure: None,
                    };
                    if let Err(e) = article.save_to_file(&feed_name) {
                        tracing::error!(
                            feed = feed_name.as_str(),
                            article = title.as_str(),
                            "save failed: {}",
                            e
                        );
                    }
                    feed_meta.add_article(&article.id, &article.published_at);
                    monitor.write().await.complete_article(&feed_name, &title);
                }
                Err(e) => {
                    tracing::error!("task join error: {}", e);
                }
            }
        }

        if let Err(e) = feed_meta.save(feed_name) {
            tracing::error!(feed = feed_name, "meta save failed: {}", e);
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;
        tracing::info!(
            feed = feed_name,
            total = feed_meta.article_ids.len(),
            new = total,
            translated = translated_count,
            failed = failed_count,
            tokens = total_tokens,
            elapsed_ms = elapsed_ms,
            "feed processed"
        );
        self.monitor
            .write()
            .await
            .set_status(feed_name, FeedStatus::Done);
    }

    pub async fn process_all(&self) {
        let cfg = self.config.read().await;
        let feeds: Vec<_> = cfg
            .feeds
            .iter()
            .filter(|f| f.enabled)
            .map(|f| (f.name.clone(), f.url.clone()))
            .collect();
        drop(cfg);

        let handles: Vec<_> = feeds
            .into_iter()
            .map(|(name, url)| {
                let scheduler = self.clone();
                tokio::spawn(async move {
                    scheduler.process_feed(&name, &url).await;
                })
            })
            .collect();

        for handle in handles {
            if let Err(e) = handle.await {
                tracing::error!("feed task join error: {}", e);
            }
        }
    }

    pub async fn run_loop(self: Arc<Self>) {
        loop {
            self.process_all().await;
            let interval = {
                let c = self.config.read().await;
                c.feeds
                    .iter()
                    .filter(|f| f.enabled)
                    .map(|f| f.interval_secs)
                    .min()
                    .unwrap_or(300)
            };
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    }
}

async fn process_single_article(
    feed_name: &str,
    raw: crate::rss::fetch::RawArticle,
    tc: &crate::config::LlmProviderConfig,
    target: &str,
    semaphore: Arc<Semaphore>,
    monitor: Arc<RwLock<Monitor>>,
    retry: RetryConfig,
) -> Result<Article, crate::error::AppError> {
    let title = raw.title.clone();
    monitor.write().await.start_article(feed_name, &title, 1);

    let source_lang = crate::lang::detect(&raw.content).or_else(|| crate::lang::detect(&raw.title));
    let needs_translation =
        !raw.content.is_empty() && crate::lang::needs_translation(&raw.content, target);
    let needs_title_translation = crate::lang::needs_translation(&raw.title, target);

    if !needs_translation && !needs_title_translation {
        return Ok(Article {
            id: raw
                .guid
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            feed_name: feed_name.to_string(),
            title: raw.title.clone(),
            original_title: raw.title,
            link: raw.link,
            content: raw.content.clone(),
            original_content: raw.content,
            summary: None,
            translated: false,
            translated_title: false,
            source_lang,
            published_at: raw.published_at.clone(),
            published_at_rfc2822: chrono::DateTime::parse_from_rfc2822(&raw.published_at)
                .ok()
                .map(|dt| dt.to_rfc2822()),
            processed_at: chrono::Utc::now().to_rfc3339(),
            author: raw.author,
            categories: raw.categories,
            translation_model: None,
            translation_tokens: None,
            enclosure: raw.media_urls.first().map(|m| Enclosure {
                url: m.url.clone(),
                content_type: m.content_type.clone(),
                length: m.length,
            }),
        });
    }

    let model = tc.model.clone();

    let _permit = semaphore.acquire().await.unwrap();

    let mut retry_ctx = RetryContext::new(
        retry.max_retries,
        retry.delay_secs,
        feed_name.to_string(),
        raw.title.clone(),
        model.clone(),
        monitor.clone(),
    );

    match crate::llm::translate_summarize::translate_and_summarize(
        tc,
        &raw.title,
        &raw.content,
        target,
        &mut retry_ctx,
    )
    .await
    {
        Ok((result, parsed)) => {
            let translated_title = parsed.title.is_some();
            let translated_content = parsed.content.is_some();

            let final_title = parsed.title.unwrap_or_else(|| raw.title.clone());
            let final_content = parsed.content.unwrap_or_else(|| raw.content.clone());
            let summary = parsed.summary;

            Ok(Article {
                id: raw
                    .guid
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
                feed_name: feed_name.to_string(),
                title: final_title,
                original_title: raw.title,
                link: raw.link,
                content: final_content,
                original_content: raw.content,
                summary,
                translated: translated_content,
                translated_title,
                source_lang,
                published_at: raw.published_at.clone(),
                published_at_rfc2822: chrono::DateTime::parse_from_rfc2822(&raw.published_at)
                    .ok()
                    .map(|dt| dt.to_rfc2822()),
                processed_at: chrono::Utc::now().to_rfc3339(),
                author: raw.author,
                categories: raw.categories,
                translation_model: if translated_content || translated_title {
                    Some(model.clone())
                } else {
                    None
                },
                translation_tokens: Some(
                    result.usage.prompt_tokens + result.usage.completion_tokens,
                ),
                enclosure: raw.media_urls.first().map(|m| Enclosure {
                    url: m.url.clone(),
                    content_type: m.content_type.clone(),
                    length: m.length,
                }),
            })
        }
        Err(e) => Err(e),
    }
}
