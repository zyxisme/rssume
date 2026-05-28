use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Serialize)]
pub struct Monitor {
    pub feeds: HashMap<String, FeedRuntimeState>,
    #[serde(skip)]
    pub translation_logs: HashMap<String, VecDeque<TranslationLog>>,
    pub token_usage: TokenUsage,
}

impl Monitor {
    pub fn new() -> Self {
        Monitor {
            feeds: HashMap::new(),
            translation_logs: HashMap::new(),
            token_usage: TokenUsage::load(),
        }
    }
    pub fn ensure_feed(&mut self, name: &str) {
        self.feeds
            .entry(name.to_string())
            .or_insert_with(|| FeedRuntimeState {
                status: FeedStatus::Idle,
                last_fetch_at: None,
                last_fetch_error: None,
                last_poll_duration_ms: 0,
            });
    }
    pub fn set_status(&mut self, name: &str, status: FeedStatus) {
        self.feeds.entry(name.to_string()).and_modify(|s| {
            s.status = status;
        });
    }
    pub fn finish_fetch(&mut self, name: &str, duration_ms: u64, error: Option<&str>) {
        self.feeds.entry(name.to_string()).and_modify(|s| {
            s.last_fetch_at = Some(chrono::Utc::now().to_rfc3339());
            s.last_poll_duration_ms = duration_ms;
            s.last_fetch_error = error.map(|e| e.to_string());
        });
    }
    pub fn add_log(&mut self, feed_name: &str, log: TranslationLog) {
        let logs = self
            .translation_logs
            .entry(feed_name.to_string())
            .or_insert_with(|| VecDeque::with_capacity(500));
        logs.push_back(log);
        while logs.len() > 500 {
            logs.pop_front();
        }
    }
    pub fn update_log(
        &mut self,
        feed_name: &str,
        log_id: &str,
        f: impl FnOnce(&mut TranslationLog),
    ) {
        if let Some(logs) = self.translation_logs.get_mut(feed_name)
            && let Some(log) = logs.iter_mut().find(|l| l.id == log_id)
        {
            f(log);
        }
    }
    pub fn add_token_usage(&mut self, feed_name: &str, model: &str, prompt: u32, completion: u32) {
        self.token_usage.total_prompt_tokens += prompt as u64;
        self.token_usage.total_completion_tokens += completion as u64;
        self.token_usage
            .by_model
            .entry(model.to_string())
            .and_modify(|u| {
                u.prompt_tokens += prompt as u64;
                u.completion_tokens += completion as u64;
                u.request_count += 1;
            })
            .or_insert_with(|| ModelUsage {
                prompt_tokens: prompt as u64,
                completion_tokens: completion as u64,
                request_count: 1,
            });
        self.token_usage
            .by_feed
            .entry(feed_name.to_string())
            .and_modify(|u| {
                u.prompt_tokens += prompt as u64;
                u.completion_tokens += completion as u64;
                u.article_count += 1;
            })
            .or_insert_with(|| FeedTokenUsage {
                prompt_tokens: prompt as u64,
                completion_tokens: completion as u64,
                article_count: 1,
            });
        self.token_usage.save();
    }
    pub fn get_logs(&self, feed_name: &str) -> Vec<&TranslationLog> {
        self.translation_logs
            .get(feed_name)
            .map(|l| l.iter().collect())
            .unwrap_or_default()
    }
    pub fn active_translations(&self) -> Vec<(String, &TranslationLog)> {
        self.translation_logs
            .iter()
            .filter_map(|(f, logs)| {
                logs.iter()
                    .rev()
                    .find(|l| matches!(l.status, LogStatus::Started | LogStatus::Streaming { .. }))
                    .map(|l| (f.clone(), l))
            })
            .collect()
    }
    pub fn start_article(&mut self, feed_name: &str, title: &str, total: u32) {
        self.feeds.entry(feed_name.to_string()).and_modify(|s| {
            if let FeedStatus::Translating {
                ref mut in_progress,
                ..
            } = s.status
            {
                in_progress.push(title.to_string());
            } else {
                s.status = FeedStatus::Translating {
                    completed: 0,
                    total,
                    in_progress: vec![title.to_string()],
                };
            }
        });
    }
    pub fn complete_article(&mut self, feed_name: &str, title: &str) {
        self.feeds.entry(feed_name.to_string()).and_modify(|s| {
            if let FeedStatus::Translating {
                ref mut completed,
                ref mut in_progress,
                ..
            } = s.status
            {
                *completed += 1;
                in_progress.retain(|t| t != title);
            }
        });
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedRuntimeState {
    pub status: FeedStatus,
    pub last_fetch_at: Option<String>,
    pub last_fetch_error: Option<String>,
    pub last_poll_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum FeedStatus {
    Idle,
    Fetching,
    Translating {
        completed: u32,
        total: u32,
        in_progress: Vec<String>,
    },
    Done,
    #[allow(dead_code)]
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct TranslationLog {
    pub id: String,
    pub timestamp: String,
    pub article_title: String,
    pub stage: TranslationStage,
    pub status: LogStatus,
    pub model: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    #[serde(skip)]
    pub streamed_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum TranslationStage {
    TranslatingTitle,
    TranslatingContent,
    Summarizing,
}

#[derive(Debug, Clone, Serialize)]
pub enum LogStatus {
    Started,
    Streaming { tokens: String },
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub by_model: HashMap<String, ModelUsage>,
    pub by_feed: HashMap<String, FeedTokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub request_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedTokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub article_count: u64,
}

impl TokenUsage {
    fn path() -> std::path::PathBuf {
        crate::config::Config::data_dir().join("token_usage.toml")
    }
    fn load() -> Self {
        let p = Self::path();
        if p.exists() {
            std::fs::read_to_string(&p)
                .ok()
                .and_then(|s| toml::from_str(&s).ok())
                .unwrap_or_else(Self::default)
        } else {
            Self::default()
        }
    }
    fn save(&self) {
        if let Ok(c) = toml::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), c);
        }
    }
    fn default() -> Self {
        TokenUsage {
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            by_model: HashMap::new(),
            by_feed: HashMap::new(),
        }
    }
}
