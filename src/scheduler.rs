use crate::config::Config;
use crate::llm::{summarize, translate};
use crate::monitor::{FeedStatus, LogStatus, Monitor, TranslationLog, TranslationStage};
use crate::rss::fetch;
use crate::storage::{Article, Enclosure, FeedData};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct Scheduler {
    config: Arc<RwLock<Config>>,
    monitor: Arc<RwLock<Monitor>>,
}

impl Scheduler {
    pub fn new(config: Arc<RwLock<Config>>, monitor: Arc<RwLock<Monitor>>) -> Self {
        Scheduler { config, monitor }
    }

    pub async fn process_feed(&self, feed_name: &str, feed_url: &str) {
        tracing::info!("Processing feed: {} ({})", feed_name, feed_url);
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
                tracing::error!("Fetch failed '{}': {}", feed_name, e);
                self.monitor
                    .write()
                    .await
                    .finish_fetch(feed_name, ms, Some(&e.to_string()));
                return;
            }
        };
        self.monitor.write().await.finish_fetch(
            feed_name,
            start.elapsed().as_millis() as u64,
            None,
        );

        let mut feed_data = match FeedData::load(feed_name) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Load failed: {}", e);
                return;
            }
        };

        let config = self.config.read().await.clone();
        let new_articles: Vec<_> = raw_articles
            .into_iter()
            .filter(|a| !feed_data.contains_link(&a.link))
            .collect();

        if new_articles.is_empty() {
            self.monitor
                .write()
                .await
                .set_status(feed_name, FeedStatus::Done);
            return;
        }

        let total = new_articles.len() as u32;
        let tc = config.llm.translation.clone();
        let sc = config.llm.summary.clone();
        let target = config.language.target.clone();

        for (i, raw) in new_articles.into_iter().enumerate() {
            self.monitor.write().await.set_status(
                feed_name,
                FeedStatus::Translating {
                    current: i as u32 + 1,
                    total,
                    current_title: raw.title.clone(),
                },
            );

            let source_lang =
                crate::lang::detect(&raw.content).or_else(|| crate::lang::detect(&raw.title));
            let needs_ct =
                !raw.content.is_empty() && crate::lang::needs_translation(&raw.content, &target);
            let needs_tt = crate::lang::needs_translation(&raw.title, &target);
            let model = tc.model.clone();
            let sum_model = sc.model.clone();
            let mut total_translation_tokens: u32 = 0;

            // ---- Title Translation ----
            let (final_title, tt) = if needs_tt {
                let log = mlog(&raw.title, TranslationStage::TranslatingTitle, &model);
                let lid = log.id.clone();
                self.monitor.write().await.add_log(feed_name, log);
                let ot = mtok(self.monitor.clone(), feed_name.to_string(), lid.clone());
                match translate::translate(&tc, &raw.title, &target, ot).await {
                    Ok(r) => {
                        let translated = r.text != raw.title;
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            if translated {
                                l.status = LogStatus::Completed;
                                l.prompt_tokens = Some(r.usage.prompt_tokens);
                                l.completion_tokens = Some(r.usage.completion_tokens);
                            } else {
                                l.status =
                                    LogStatus::Failed("model returned untranslated text".into());
                            }
                        });
                        if translated {
                            total_translation_tokens +=
                                r.usage.prompt_tokens + r.usage.completion_tokens;
                            self.monitor.write().await.add_token_usage(
                                feed_name,
                                &model,
                                r.usage.prompt_tokens,
                                r.usage.completion_tokens,
                            );
                        }
                        (
                            if translated {
                                r.text
                            } else {
                                raw.title.clone()
                            },
                            translated,
                        )
                    }
                    Err(e) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            l.status = LogStatus::Failed(e.to_string());
                        });
                        (raw.title.clone(), false)
                    }
                }
            } else {
                (raw.title.clone(), false)
            };

            // ---- Content Translation ----
            let (final_content, ct) = if needs_ct {
                let log = mlog(&raw.title, TranslationStage::TranslatingContent, &model);
                let lid = log.id.clone();
                self.monitor.write().await.add_log(feed_name, log);
                let ot = mtok(self.monitor.clone(), feed_name.to_string(), lid.clone());
                match translate::translate(&tc, &raw.content, &target, ot).await {
                    Ok(r) => {
                        let translated = r.text != raw.content;
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            if translated {
                                l.status = LogStatus::Completed;
                                l.prompt_tokens = Some(r.usage.prompt_tokens);
                                l.completion_tokens = Some(r.usage.completion_tokens);
                            } else {
                                l.status =
                                    LogStatus::Failed("model returned untranslated text".into());
                            }
                        });
                        if translated {
                            total_translation_tokens +=
                                r.usage.prompt_tokens + r.usage.completion_tokens;
                            self.monitor.write().await.add_token_usage(
                                feed_name,
                                &model,
                                r.usage.prompt_tokens,
                                r.usage.completion_tokens,
                            );
                        }
                        (
                            if translated {
                                r.text
                            } else {
                                raw.content.clone()
                            },
                            translated,
                        )
                    }
                    Err(e) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            l.status = LogStatus::Failed(e.to_string());
                        });
                        (raw.content.clone(), false)
                    }
                }
            } else {
                (raw.content.clone(), false)
            };

            // ---- Summarization ----
            let summary = {
                let log = mlog(&final_title, TranslationStage::Summarizing, &sum_model);
                let lid = log.id.clone();
                self.monitor.write().await.add_log(feed_name, log);
                let ot = mtok(self.monitor.clone(), feed_name.to_string(), lid.clone());
                match summarize::summarize(&sc, &final_title, &final_content, ot).await {
                    Ok(r) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            l.status = LogStatus::Completed;
                            l.prompt_tokens = Some(r.usage.prompt_tokens);
                            l.completion_tokens = Some(r.usage.completion_tokens);
                        });
                        self.monitor.write().await.add_token_usage(
                            feed_name,
                            &sum_model,
                            r.usage.prompt_tokens,
                            r.usage.completion_tokens,
                        );
                        Some(r.text)
                    }
                    Err(e) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            l.status = LogStatus::Failed(e.to_string());
                        });
                        None
                    }
                }
            };

            let enclosure = raw.media_urls.first().map(|m| Enclosure {
                url: m.url.clone(),
                content_type: m.content_type.clone(),
                length: m.length,
            });

            let translation_tokens = if total_translation_tokens > 0 {
                Some(total_translation_tokens)
            } else {
                None
            };
            let article = Article {
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
                translated: ct,
                translated_title: tt,
                source_lang,
                published_at: raw.published_at.clone(),
                published_at_rfc2822: chrono::DateTime::parse_from_rfc2822(&raw.published_at)
                    .ok()
                    .map(|dt| dt.to_rfc2822()),
                processed_at: chrono::Utc::now().to_rfc3339(),
                author: raw.author,
                categories: raw.categories,
                translation_model: if ct || tt { Some(model.clone()) } else { None },
                translation_tokens,
                enclosure,
            };
            feed_data.articles.push(article);

            if let Err(e) = feed_data.save(feed_name) {
                tracing::error!("Save failed '{}': {}", feed_name, e);
            }
        }

        feed_data
            .articles
            .sort_by(|a, b| b.published_at.cmp(&a.published_at));

        tracing::info!(
            "Feed '{}' processed: {} total",
            feed_name,
            feed_data.article_count()
        );
        self.monitor
            .write()
            .await
            .set_status(feed_name, FeedStatus::Done);
    }

    pub async fn process_all(&self) {
        let cfg = self.config.read().await;
        for f in &cfg.feeds {
            if f.enabled {
                self.process_feed(&f.name, &f.url).await;
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

fn mlog(title: &str, stage: TranslationStage, model: &str) -> TranslationLog {
    TranslationLog {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        article_title: title.to_string(),
        stage,
        status: LogStatus::Started,
        model: model.to_string(),
        prompt_tokens: None,
        completion_tokens: None,
        streamed_text: String::new(),
    }
}

fn mtok(monitor: Arc<RwLock<Monitor>>, feed: String, lid: String) -> impl FnMut(&str) {
    move |t: &str| {
        let m = monitor.clone();
        let f = feed.clone();
        let l = lid.clone();
        let s = t.to_string();
        tokio::task::spawn(async move {
            m.write().await.update_log(&f, &l, |log| {
                log.streamed_text.push_str(&s);
                log.status = LogStatus::Streaming {
                    tokens: log.streamed_text.clone(),
                };
            });
        });
    }
}
