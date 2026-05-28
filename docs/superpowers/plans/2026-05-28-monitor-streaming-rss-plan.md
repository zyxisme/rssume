# Monitor + Streaming LLM + RSS Enhancement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add real-time translation monitoring, streaming LLM calls with title translation, token usage tracking, and RSS format compatibility enhancements.

**Architecture:** New `monitor.rs` module provides `Arc<RwLock<Monitor>>` as shared state. LLM `chat()` → `chat_stream()` with SSE parsing and `on_token` callback. Panel pages use htmx polling against `/api/monitor/*` for live status. `AppState { config, monitor }` injected into all web routes.

**Dep changes:** reqwest +`stream` feature, add `futures-util = "0.3"`

**Impl order (12 tasks):** monitor → llm streaming → translate/summarize → rss fetch → rss generate → storage → scheduler → web/api → web/mod+panel → templates(3 new + 2 modified) → main.rs

---

### Task 1: Create monitor.rs — shared state structures

**Files:** Create: `src/monitor.rs` / Modify: `src/main.rs:1-9`

- [ ] **Step 1: Write `src/monitor.rs`**

```rust
use std::collections::{HashMap, VecDeque};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Monitor {
    pub feeds: HashMap<String, FeedRuntimeState>,
    #[serde(skip)]
    pub translation_logs: HashMap<String, VecDeque<TranslationLog>>,
    pub token_usage: TokenUsage,
}

impl Monitor {
    pub fn new() -> Self {
        Monitor { feeds: HashMap::new(), translation_logs: HashMap::new(), token_usage: TokenUsage::load() }
    }
    pub fn ensure_feed(&mut self, name: &str) {
        self.feeds.entry(name.to_string()).or_insert_with(|| FeedRuntimeState {
            status: FeedStatus::Idle, last_fetch_at: None, last_fetch_error: None, last_poll_duration_ms: 0,
        });
    }
    pub fn set_status(&mut self, name: &str, status: FeedStatus) {
        self.feeds.entry(name.to_string()).and_modify(|s| { s.status = status; });
    }
    pub fn finish_fetch(&mut self, name: &str, duration_ms: u64, error: Option<&str>) {
        self.feeds.entry(name.to_string()).and_modify(|s| {
            s.last_fetch_at = Some(chrono::Utc::now().to_rfc3339());
            s.last_poll_duration_ms = duration_ms;
            s.last_fetch_error = error.map(|e| e.to_string());
        });
    }
    pub fn add_log(&mut self, feed_name: &str, log: TranslationLog) {
        let logs = self.translation_logs.entry(feed_name.to_string())
            .or_insert_with(|| VecDeque::with_capacity(500));
        logs.push_back(log);
        while logs.len() > 500 { logs.pop_front(); }
    }
    pub fn update_log(&mut self, feed_name: &str, log_id: &str, f: impl FnOnce(&mut TranslationLog)) {
        if let Some(logs) = self.translation_logs.get_mut(feed_name) {
            if let Some(log) = logs.iter_mut().find(|l| l.id == log_id) { f(log); }
        }
    }
    pub fn add_token_usage(&mut self, feed_name: &str, model: &str, prompt: u32, completion: u32) {
        self.token_usage.total_prompt_tokens += prompt as u64;
        self.token_usage.total_completion_tokens += completion as u64;
        self.token_usage.by_model.entry(model.to_string())
            .and_modify(|u| { u.prompt_tokens += prompt as u64; u.completion_tokens += completion as u64; u.request_count += 1; })
            .or_insert_with(|| ModelUsage { prompt_tokens: prompt as u64, completion_tokens: completion as u64, request_count: 1 });
        self.token_usage.by_feed.entry(feed_name.to_string())
            .and_modify(|u| { u.prompt_tokens += prompt as u64; u.completion_tokens += completion as u64; u.article_count += 1; })
            .or_insert_with(|| FeedTokenUsage { prompt_tokens: prompt as u64, completion_tokens: completion as u64, article_count: 1 });
        self.token_usage.save();
    }
    pub fn get_logs(&self, feed_name: &str) -> Vec<&TranslationLog> {
        self.translation_logs.get(feed_name).map(|l| l.iter().collect()).unwrap_or_default()
    }
    pub fn active_translations(&self) -> Vec<(String, &TranslationLog)> {
        self.translation_logs.iter().filter_map(|(f, logs)| {
            logs.iter().rev().find(|l| matches!(l.status, LogStatus::Started | LogStatus::Streaming { .. })).map(|l| (f.clone(), l))
        }).collect()
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
    Idle, Fetching,
    Translating { current: u32, total: u32, current_title: String },
    Done, Error(String),
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
    #[serde(skip)] pub streamed_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum TranslationStage { TranslatingTitle, TranslatingContent, Summarizing }

#[derive(Debug, Clone, Serialize)]
pub enum LogStatus {
    Started, Streaming { tokens: String }, Completed, Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub by_model: HashMap<String, ModelUsage>,
    pub by_feed: HashMap<String, FeedTokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage { pub prompt_tokens: u64, pub completion_tokens: u64, pub request_count: u64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedTokenUsage { pub prompt_tokens: u64, pub completion_tokens: u64, pub article_count: u64 }

impl TokenUsage {
    fn path() -> std::path::PathBuf { crate::config::Config::data_dir().join("token_usage.toml") }
    fn load() -> Self {
        let p = Self::path();
        if p.exists() { std::fs::read_to_string(&p).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default() }
        else { Self::default() }
    }
    fn save(&self) {
        if let Ok(c) = toml::to_string_pretty(self) { let _ = std::fs::write(Self::path(), c); }
    }
    fn default() -> Self {
        TokenUsage { total_prompt_tokens: 0, total_completion_tokens: 0, by_model: HashMap::new(), by_feed: HashMap::new() }
    }
}
```

- [ ] **Step 2: Add `mod monitor;`** to `src/main.rs` after `mod llm;`

- [ ] **Step 3: Build check** — `cargo check 2>&1` (unused module ok)

- [ ] **Step 4: Commit**

---

### Task 2: LLM streaming — chat_stream() with SSE

**Files:** `Cargo.toml` (features), `src/llm/mod.rs` (full rewrite)

- [ ] **Step 1: Cargo.toml** — change reqwest: `features = ["json", "rustls-tls", "stream"]`, add `futures-util = "0.3"`

- [ ] **Step 2: Rewrite `src/llm/mod.rs`**

```rust
pub mod summarize;
pub mod translate;

use crate::config::LlmProviderConfig;
use futures_util::StreamExt;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ChatMessage { role: String, content: String }

#[derive(Debug, Serialize)]
struct ChatRequest { model: String, messages: Vec<ChatMessage>, temperature: f32, max_tokens: u32, stream: bool }

#[derive(Debug, serde::Deserialize)]
struct StreamChunk { choices: Vec<StreamChoice>, usage: Option<UsageInfo> }

#[derive(Debug, serde::Deserialize)]
struct StreamChoice { delta: StreamDelta }

#[derive(Debug, serde::Deserialize)]
struct StreamDelta { content: Option<String> }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UsageInfo { pub prompt_tokens: u32, pub completion_tokens: u32, pub total_tokens: u32 }

pub struct StreamResult { pub text: String, pub usage: UsageInfo }

pub async fn chat(config: &LlmProviderConfig, system_prompt: &str, user_prompt: &str) -> Result<StreamResult, crate::error::AppError> {
    chat_stream(config, system_prompt, user_prompt, |_| {}).await
}

pub async fn chat_stream(
    config: &LlmProviderConfig, system_prompt: &str, user_prompt: &str,
    mut on_token: impl FnMut(&str),
) -> Result<StreamResult, crate::error::AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build().map_err(|e| crate::error::AppError::Llm(format!("client: {}", e)))?;

    let body = ChatRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage { role: "system".into(), content: system_prompt.into() },
            ChatMessage { role: "user".into(), content: user_prompt.into() },
        ],
        temperature: 0.3, max_tokens: 4096, stream: true,
    };

    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let resp = client.post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body).send().await
        .map_err(|e| crate::error::AppError::Llm(format!("request: {}", e)))?;

    if !resp.status().is_success() {
        let s = resp.status(); let t = resp.text().await.unwrap_or_default();
        return Err(crate::error::AppError::Llm(format!("API {}: {}", s, t)));
    }

    let mut stream = resp.bytes_stream();
    let mut full_text = String::new();
    let mut usage: Option<UsageInfo> = None;
    let mut buf = String::new();

    loop {
        let chunk = match tokio::time::timeout(std::time::Duration::from_secs(60), stream.next()).await {
            Ok(Some(Ok(b))) => b,
            Ok(Some(Err(e))) => {
                if full_text.is_empty() { return Err(crate::error::AppError::Llm(format!("stream: {}", e))); }
                break;
            }
            Ok(None) => break,
            Err(_) => {
                if full_text.is_empty() { return Err(crate::error::AppError::Llm("idle timeout".into())); }
                break;
            }
        };

        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string(); buf = buf[nl+1..].to_string();
            if line.is_empty() || line.starts_with(':') { continue; }
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" { continue; }
                if let Ok(c) = serde_json::from_str::<StreamChunk>(data) {
                    if let Some(ref d) = c.choices.first().map(|c| &c.delta) {
                        if let Some(ref ct) = d.content { full_text.push_str(ct); on_token(ct); }
                    }
                    if let Some(u) = c.usage { usage = Some(u); }
                }
            }
        }
    }

    let usage = usage.unwrap_or(UsageInfo { prompt_tokens: 0, completion_tokens: full_text.len() as u32 / 4, total_tokens: 0 });
    Ok(StreamResult { text: full_text, usage })
}
```

- [ ] **Step 3: Verify** — `cargo check 2>&1` — errors in translate.rs/summarize.rs (return type `String`→`StreamResult`). Expected.

- [ ] **Step 4: Commit**

---

### Task 3: Adapt translate.rs + summarize.rs to streaming API

**Files:** `src/llm/translate.rs`, `src/llm/summarize.rs`

- [ ] **Step 1: `src/llm/translate.rs`** — change signature, add `on_token` param, return `StreamResult`:

```rust
use crate::config::LlmProviderConfig;
use super::{chat_stream, StreamResult};

const SYSTEM_PROMPT: &str = r#"You are a professional translator. Translate the following text accurately into the target language.
Preserve all formatting, HTML tags, code blocks, and technical terms.
Only output the translated text, nothing else."#;

pub async fn translate(
    config: &LlmProviderConfig, text: &str, target_lang: &str,
    on_token: impl FnMut(&str),
) -> Result<StreamResult, crate::error::AppError> {
    let lang_name = crate::lang::lang_name(target_lang);
    let prompt = format!("Translate the following text to {}:\n\n{}", lang_name, text);
    let append = config.prompt_append.clone().unwrap_or_default();
    let full = if append.is_empty() { prompt } else { format!("{}\n{}", prompt, append) };
    chat_stream(config, SYSTEM_PROMPT, &full, on_token).await
}
```

- [ ] **Step 2: `src/llm/summarize.rs`** — same pattern:

```rust
use crate::config::LlmProviderConfig;
use super::{chat_stream, StreamResult};

const SYSTEM_PROMPT: &str = r#"You are a skilled content summarizer. Write a concise one-sentence summary of the following article in Chinese.
Capture the key point. Output only the summary, nothing else."#;

pub async fn summarize(
    config: &LlmProviderConfig, title: &str, content: &str,
    on_token: impl FnMut(&str),
) -> Result<StreamResult, crate::error::AppError> {
    let text = if content.len() > 4000 { &content[..content.floor_char_boundary(4000)] } else { content };
    let prompt = format!("Title: {}\n\nContent:\n{}", title, text);
    let append = config.prompt_append.clone().unwrap_or_default();
    let full = if append.is_empty() { prompt } else { format!("{}\n{}", prompt, append) };
    chat_stream(config, SYSTEM_PROMPT, &full, on_token).await
}
```

- [ ] **Step 3: Verify** — `cargo check 2>&1` — errors only in scheduler.rs. Expected.

- [ ] **Step 4: Commit**

---

### Task 4: Extend RSS fetch — author, categories, guid, media

**Files:** `src/rss/fetch.rs`

- [ ] **Step 1: Add structs + rewrite `fetch_feed`**

```rust
use chrono::Utc;
use feed_rs::parser;

#[derive(Debug, Clone)]
pub struct RawArticle {
    pub title: String, pub link: String, pub content: String, pub published_at: String,
    pub author: Option<String>, pub categories: Vec<String>, pub guid: Option<String>,
    pub media_urls: Vec<MediaItem>,
}

#[derive(Debug, Clone)]
pub struct MediaItem { pub url: String, pub content_type: Option<String>, pub length: Option<u64> }

pub async fn fetch_feed(url: &str) -> Result<Vec<RawArticle>, crate::error::AppError> {
    let client = reqwest::Client::builder()
        .user_agent("rssume/0.1 (RSS middleware; +https://github.com/rssume/rssume)")
        .timeout(std::time::Duration::from_secs(30)).build()
        .map_err(|e| crate::error::AppError::Fetch(format!("client: {}", e)))?;

    let resp = client.get(url).send().await
        .map_err(|e| crate::error::AppError::Fetch(format!("fetch {}: {}", url, e)))?;
    if !resp.status().is_success() {
        return Err(crate::error::AppError::Fetch(format!("HTTP {} for {}", resp.status(), url)));
    }

    let body = resp.bytes().await
        .map_err(|e| crate::error::AppError::Fetch(format!("body: {}", e)))?;
    let feed = parser::parse(&body[..])
        .map_err(|e| crate::error::AppError::Parse(format!("parse: {}", e)))?;

    let articles: Vec<_> = feed.entries.into_iter().map(|entry| {
        let content = entry.content.as_ref().and_then(|c| c.body.as_ref()).cloned().unwrap_or_default();
        let summary = entry.summary.as_ref().map(|s| s.content.clone()).unwrap_or_default();
        let body = if !content.is_empty() { content } else { summary };
        let media_urls = entry.media.iter().flat_map(|m| &m.content)
            .filter_map(|mc| mc.url.as_ref().map(|u| MediaItem {
                url: u.to_string(), content_type: mc.content_type.as_ref().map(|t| t.to_string()), length: mc.size,
            })).collect();

        RawArticle {
            title: entry.title.as_ref().map(|t| t.content.clone()).unwrap_or_default(),
            link: entry.links.first().map(|l| l.href.clone()).unwrap_or_default(),
            content: body,
            published_at: entry.published.or(entry.updated)
                .map(|d| d.to_rfc2822()).unwrap_or_else(|| Utc::now().to_rfc2822()),
            author: entry.authors.first().map(|p| p.name.clone()),
            categories: entry.categories.into_iter().map(|c| c.term).collect(),
            guid: Some(entry.id),
            media_urls,
        }
    }).filter(|a| !a.link.is_empty() && !a.title.is_empty()).collect();

    Ok(articles)
}
```

- [ ] **Step 2: Verify** — `cargo check 2>&1` — errors about `RawArticle` fields changed in scheduler/generate.

- [ ] **Step 3: Commit**

---

### Task 5: RSS generate — content:encoded, namespaces, dc:creator, enclosure

**Files:** `src/rss/generate.rs`

- [ ] **Step 1: Rewrite with namespaces and new fields**

```rust
use crate::storage::Article;
use chrono::DateTime;

pub fn generate_rss(feed_name: &str, articles: &[Article]) -> String {
    let now = chrono::Utc::now().to_rfc2822();
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(concat!(
        r#"<rss version="2.0""#,
        r#" xmlns:atom="http://www.w3.org/2005/Atom""#,
        r#" xmlns:content="http://purl.org/rss/1.0/modules/content/""#,
        r#" xmlns:dc="http://purl.org/dc/elements/1.1/""#,
        r#">"#,
    ));
    xml.push('\n');
    xml.push_str("  <channel>\n");
    xml.push_str(&format!("    <title>{}</title>\n", esc(feed_name)));
    xml.push_str(&format!("    <description>rssume processed feed for {}</description>\n", esc(feed_name)));
    xml.push_str(&format!("    <link>/feeds/{}</link>\n", esc(feed_name)));
    xml.push_str(&format!("    <atom:link href=\"/feeds/{}\" rel=\"self\" type=\"application/rss+xml\"/>\n", esc(feed_name)));
    xml.push_str(&format!("    <lastBuildDate>{}</lastBuildDate>\n", now));
    xml.push_str("    <generator>rssume</generator>\n");

    for article in articles.iter().rev().take(50) {
        xml.push_str("    <item>\n");
        xml.push_str(&format!("      <title>{}</title>\n", esc(&article.title)));

        if let Some(ref a) = article.author {
            xml.push_str(&format!("      <dc:creator>{}</dc:creator>\n", esc(a)));
        }
        for c in &article.categories {
            xml.push_str(&format!("      <category>{}</category>\n", esc(c)));
        }

        let desc = if let Some(ref s) = article.summary {
            format!("[AI 摘要] {}", esc(s))
        } else {
            esc(&article.content.chars().take(200).collect::<String>())
        };
        xml.push_str(&format!("      <description>{}</description>\n", desc));

        xml.push_str("      <content:encoded><![CDATA[");
        if let Some(ref s) = article.summary {
            xml.push_str(&format!("<p><strong>[AI 摘要]</strong> {}</p><hr/>", esc(s)));
        }
        xml.push_str(&article.content);
        if article.translated {
            xml.push_str(&format!("<p><em>(Translated from {})</em></p>",
                article.source_lang.as_ref().map(|l| crate::lang::lang_name(l)).unwrap_or_else(|| "unknown".to_string())));
        }
        xml.push_str("]]></content:encoded>\n");

        xml.push_str(&format!("      <link>{}</link>\n", esc(&article.link)));
        xml.push_str(&format!("      <guid isPermaLink=\"false\">{}</guid>\n", esc(&article.id)));

        if let Some(ref d) = article.published_at_rfc2822 {
            xml.push_str(&format!("      <pubDate>{}</pubDate>\n", d));
        } else if let Ok(dt) = DateTime::parse_from_rfc2822(&article.published_at) {
            xml.push_str(&format!("      <pubDate>{}</pubDate>\n", dt.to_rfc2822()));
        }

        if let Some(ref enc) = article.enclosure {
            xml.push_str(&format!("      <enclosure url=\"{}\" length=\"{}\" type=\"{}\"/>\n",
                esc(&enc.url), enc.length.unwrap_or(0),
                esc(enc.content_type.as_deref().unwrap_or("image/jpeg"))));
        }

        xml.push_str("    </item>\n");
    }
    xml.push_str("  </channel>\n</rss>\n");
    xml
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&apos;")
}
```

- [ ] **Step 2: Verify** — `cargo check 2>&1` — errors about missing Article fields. Will be added in Task 6.

- [ ] **Step 3: Commit**

---

### Task 6: Extend storage — Article new fields + FeedStats

**Files:** `src/storage.rs`

- [ ] **Step 1: Add fields to Article + Enclosure struct, update all_feed_stats to skip token_usage.toml**

```rust
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
```

- [ ] **Step 2: Verify** — `cargo check 2>&1` — should compile now (might have scheduler errors still).

- [ ] **Step 3: Commit**

---

### Task 7: Integrate Monitor into scheduler — title translation, status, token tracking

**Files:** `src/scheduler.rs`

- [ ] **Step 1: Full rewrite to accept Monitor, translate titles, track status/tokens**

See the complete file below. Key changes from original:
- `Scheduler::new(config, monitor)` — takes `Arc<RwLock<Monitor>>`
- `process_feed` updates monitor status at each stage
- Title translation: if `needs_translation(&raw.title, &target)`, translates title with separate LLM call
- `make_on_token` helper: clones `Arc<Monitor>`, spawns `tokio::task` to update streamed_text in log
- `make_log` helper: creates a `TranslationLog` with `Started` status
- Token usage added via `monitor.add_token_usage()` after each successful LLM call
- `Enclosure` created from `raw.media_urls.first()`
- Article includes all new fields: `author`, `categories`, `translated_title`, `enclosure`, `published_at_rfc2822`

```rust
use crate::config::Config;
use crate::llm::{summarize, translate};
use crate::monitor::{FeedStatus, Monitor, TranslationLog, TranslationStage, LogStatus};
use crate::rss::fetch;
use crate::storage::{Article, Enclosure, FeedData};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct Scheduler { config: Arc<RwLock<Config>>, monitor: Arc<RwLock<Monitor>> }

impl Scheduler {
    pub fn new(config: Arc<RwLock<Config>>, monitor: Arc<RwLock<Monitor>>) -> Self { Scheduler { config, monitor } }

    pub async fn process_feed(&self, feed_name: &str, feed_url: &str) {
        tracing::info!("Processing feed: {} ({})", feed_name, feed_url);
        let start = std::time::Instant::now();
        self.monitor.write().await.ensure_feed(feed_name);
        self.monitor.write().await.set_status(feed_name, FeedStatus::Fetching);

        let raw_articles = match fetch::fetch_feed(feed_url).await {
            Ok(a) => a,
            Err(e) => {
                let ms = start.elapsed().as_millis() as u64;
                tracing::error!("Fetch failed '{}': {}", feed_name, e);
                self.monitor.write().await.finish_fetch(feed_name, ms, Some(&e.to_string()));
                return;
            }
        };
        self.monitor.write().await.finish_fetch(feed_name, start.elapsed().as_millis() as u64, None);

        let mut feed_data = match FeedData::load(feed_name) {
            Ok(d) => d, Err(e) => { tracing::error!("Load failed: {}", e); return; }
        };

        let config = self.config.read().await.clone();
        let new_articles: Vec<_> = raw_articles.into_iter()
            .filter(|a| !feed_data.contains_link(&a.link)).collect();

        if new_articles.is_empty() {
            self.monitor.write().await.set_status(feed_name, FeedStatus::Done);
            return;
        }

        let total = new_articles.len() as u32;
        let tc = config.llm.translation.clone();
        let sc = config.llm.summary.clone();
        let target = config.language.target.clone();

        for (i, raw) in new_articles.into_iter().enumerate() {
            self.monitor.write().await.set_status(feed_name, FeedStatus::Translating {
                current: i as u32 + 1, total, current_title: raw.title.clone(),
            });

            let source_lang = crate::lang::detect(&raw.content).or_else(|| crate::lang::detect(&raw.title));
            let needs_ct = !raw.content.is_empty() && crate::lang::needs_translation(&raw.content, &target);
            let needs_tt = crate::lang::needs_translation(&raw.title, &target);
            let model = tc.model.clone();
            let sum_model = sc.model.clone();

            // Title translation
            let (final_title, tt) = if needs_tt {
                let log = mlog(&raw.title, TranslationStage::TranslatingTitle, &model);
                let lid = log.id.clone();
                self.monitor.write().await.add_log(feed_name, log);
                let ot = mtok(self.monitor.clone(), feed_name.to_string(), lid.clone());
                match translate::translate(&tc, &raw.title, &target, ot).await {
                    Ok(r) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            l.status = LogStatus::Completed; l.prompt_tokens = Some(r.usage.prompt_tokens);
                            l.completion_tokens = Some(r.usage.completion_tokens);
                        });
                        self.monitor.write().await.add_token_usage(feed_name, &model, r.usage.prompt_tokens, r.usage.completion_tokens);
                        (r.text, true)
                    }
                    Err(e) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| { l.status = LogStatus::Failed(e.to_string()); });
                        (raw.title.clone(), false)
                    }
                }
            } else { (raw.title.clone(), false) };

            // Content translation
            let (final_content, ct) = if needs_ct {
                let log = mlog(&raw.title, TranslationStage::TranslatingContent, &model);
                let lid = log.id.clone();
                self.monitor.write().await.add_log(feed_name, log);
                let ot = mtok(self.monitor.clone(), feed_name.to_string(), lid.clone());
                match translate::translate(&tc, &raw.content, &target, ot).await {
                    Ok(r) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            l.status = LogStatus::Completed; l.prompt_tokens = Some(r.usage.prompt_tokens);
                            l.completion_tokens = Some(r.usage.completion_tokens);
                        });
                        self.monitor.write().await.add_token_usage(feed_name, &model, r.usage.prompt_tokens, r.usage.completion_tokens);
                        (r.text, true)
                    }
                    Err(e) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| { l.status = LogStatus::Failed(e.to_string()); });
                        (raw.content.clone(), false)
                    }
                }
            } else { (raw.content.clone(), false) };

            // Summarize
            let summary = {
                let log = mlog(&final_title, TranslationStage::Summarizing, &sum_model);
                let lid = log.id.clone();
                self.monitor.write().await.add_log(feed_name, log);
                let ot = mtok(self.monitor.clone(), feed_name.to_string(), lid.clone());
                match summarize::summarize(&sc, &final_title, &final_content, ot).await {
                    Ok(r) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| {
                            l.status = LogStatus::Completed; l.prompt_tokens = Some(r.usage.prompt_tokens);
                            l.completion_tokens = Some(r.usage.completion_tokens);
                        });
                        self.monitor.write().await.add_token_usage(feed_name, &sum_model, r.usage.prompt_tokens, r.usage.completion_tokens);
                        Some(r.text)
                    }
                    Err(e) => {
                        self.monitor.write().await.update_log(feed_name, &lid, |l| { l.status = LogStatus::Failed(e.to_string()); });
                        None
                    }
                }
            };

            let enclosure = raw.media_urls.first().map(|m| Enclosure {
                url: m.url.clone(), content_type: m.content_type.clone(), length: m.length,
            });

            let article = Article {
                id: Uuid::new_v4().to_string(), feed_name: feed_name.to_string(),
                title: final_title, original_title: raw.title,
                link: raw.link, content: final_content, original_content: raw.content,
                summary, translated: ct, translated_title: tt, source_lang,
                published_at: raw.published_at.clone(),
                published_at_rfc2822: chrono::DateTime::parse_from_rfc2822(&raw.published_at).ok().map(|dt| dt.to_rfc2822()),
                processed_at: chrono::Utc::now().to_rfc3339(),
                author: raw.author, categories: raw.categories, enclosure,
            };
            feed_data.articles.push(article);
        }

        if let Err(e) = feed_data.save(feed_name) {
            tracing::error!("Save failed '{}': {}", feed_name, e);
        } else {
            tracing::info!("Feed '{}' processed: {} total", feed_name, feed_data.article_count());
        }
        self.monitor.write().await.set_status(feed_name, FeedStatus::Done);
    }

    pub async fn process_all(&self) {
        let cfg = self.config.read().await;
        for f in &cfg.feeds { if f.enabled { self.process_feed(&f.name, &f.url).await; } }
    }

    pub async fn run_loop(self: Arc<Self>) {
        loop {
            self.process_all().await;
            let interval = { let c = self.config.read().await; c.feeds.iter().filter(|f| f.enabled).map(|f| f.interval_secs).min().unwrap_or(300) };
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    }
}

fn mlog(title: &str, stage: TranslationStage, model: &str) -> TranslationLog {
    TranslationLog { id: Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().to_rfc3339(),
        article_title: title.to_string(), stage, status: LogStatus::Started, model: model.to_string(),
        prompt_tokens: None, completion_tokens: None, streamed_text: String::new() }
}

fn mtok(monitor: Arc<RwLock<Monitor>>, feed: String, lid: String) -> impl FnMut(&str) {
    move |t: &str| {
        let m = monitor.clone(); let f = feed.clone(); let l = lid.clone(); let s = t.to_string();
        tokio::task::spawn(async move {
            m.write().await.update_log(&f, &l, |log| {
                log.streamed_text.push_str(&s);
                log.status = LogStatus::Streaming { tokens: log.streamed_text.clone() };
            });
        });
    }
}
```

- [ ] **Step 2: Verify** — `cargo check 2>&1` — should pass.

- [ ] **Step 3: Commit**

---

### Task 8: Web API — add monitor + token endpoints, AppState

**Files:** `src/web/api.rs`

- [ ] **Step 1: Rewrite with AppState and new endpoints**

```rust
use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::monitor::{Monitor, LogStatus};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<crate::config::Config>>,
    pub monitor: Arc<RwLock<Monitor>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/feeds", get(list_feeds))
        .route("/api/monitor/status", get(monitor_status))
        .route("/api/monitor/translating", get(monitor_translating))
        .route("/api/monitor/logs/{name}", get(monitor_logs))
        .route("/api/token-usage", get(token_usage))
        .with_state(state)
}

#[derive(Serialize)]
struct ApiStats {
    feeds: Vec<crate::storage::FeedStats>, total_articles: usize,
    total_translated: usize, total_with_summary: usize,
    total_prompt_tokens: u64, total_completion_tokens: u64,
}

async fn get_stats(State(s): State<Arc<AppState>>) -> Result<Json<ApiStats>, crate::error::AppError> {
    let stats = crate::storage::all_feed_stats()?;
    let tu = &s.monitor.read().await.token_usage;
    Ok(Json(ApiStats {
        total_articles: stats.iter().map(|x| x.article_count).sum(),
        total_translated: stats.iter().map(|x| x.translated_count).sum(),
        total_with_summary: stats.iter().map(|x| x.with_summary_count).sum(),
        total_prompt_tokens: tu.total_prompt_tokens,
        total_completion_tokens: tu.total_completion_tokens,
        feeds: stats,
    }))
}

#[derive(Serialize)]
struct FeedInfo { name: String, url: String, enabled: bool, interval_secs: u64 }

async fn list_feeds(State(s): State<Arc<AppState>>) -> Json<Vec<FeedInfo>> {
    let c = s.config.read().await;
    Json(c.feeds.iter().map(|f| FeedInfo { name: f.name.clone(), url: f.url.clone(), enabled: f.enabled, interval_secs: f.interval_secs }).collect())
}

async fn monitor_status(State(s): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    let mon = s.monitor.read().await; let cfg = s.config.read().await;
    Json(cfg.feeds.iter().map(|f| {
        let rt = mon.feeds.get(&f.name);
        let d = crate::storage::FeedData::load(&f.name).ok();
        serde_json::json!({
            "name": f.name, "url": f.url, "enabled": f.enabled,
            "status": rt.map(|r| &r.status),
            "last_fetch_at": rt.and_then(|r| r.last_fetch_at.as_ref()),
            "last_fetch_error": rt.and_then(|r| r.last_fetch_error.as_ref()),
            "last_poll_duration_ms": rt.map(|r| r.last_poll_duration_ms).unwrap_or(0),
            "articles": d.as_ref().map(|d| d.article_count()).unwrap_or(0),
            "translated": d.as_ref().map(|d| d.translated_count()).unwrap_or(0),
            "summarized": d.as_ref().map(|d| d.with_summary_count()).unwrap_or(0),
        })
    }).collect())
}

async fn monitor_translating(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mon = s.monitor.read().await;
    let cfg = s.config.read().await;
    let feeds_status: Vec<_> = cfg.feeds.iter().filter_map(|f| {
        mon.feeds.get(&f.name).map(|s| (f.name.clone(), s.status.clone()))
    }).collect();
    let active = mon.active_translations();
    let recent_count: usize = mon.translation_logs.values()
        .map(|l| l.iter().filter(|l| matches!(l.status, LogStatus::Completed | LogStatus::Failed(_))).count()).sum();
    Json(serde_json::json!({
        "feeds_status": feeds_status,
        "active": active.iter().map(|(f, l)| serde_json::json!({"feed_name": f, "log": l})).collect::<Vec<_>>(),
        "recent_count": recent_count,
    }))
}

async fn monitor_logs(
    State(s): State<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Json<Vec<crate::monitor::TranslationLog>> {
    Json(s.monitor.read().await.get_logs(&name).into_iter().cloned().collect())
}

async fn token_usage(State(s): State<Arc<AppState>>) -> Json<crate::monitor::TokenUsage> {
    Json(s.monitor.read().await.token_usage.clone())
}
```

- [ ] **Step 2: Verify** — `cargo check 2>&1`.

- [ ] **Step 3: Commit**

---

### Task 9: Web panel + mod — fix dashboard config, add monitor/logs routes

**Files:** `src/web/mod.rs`, `src/web/panel.rs`

- [ ] **Step 1: Rewrite `src/web/mod.rs`**

```rust
pub mod api; pub mod panel; pub mod rss_route;

use api::AppState;
use axum::Router;
use std::sync::Arc;

pub fn router(
    config: Arc<tokio::sync::RwLock<crate::config::Config>>,
    monitor: Arc<tokio::sync::RwLock<crate::monitor::Monitor>>,
) -> Router {
    let state = Arc::new(AppState { config: config.clone(), monitor: monitor.clone() });
    Router::new()
        .merge(panel::router(state.clone()))
        .merge(api::router(state.clone()))
        .merge(rss_route::router(config))
}
```

- [ ] **Step 2: Rewrite `src/web/panel.rs`** — add `monitor_page`, `feed_logs_page` routes; dashboard reads from `state.config` (no more `Config::load()` from disk), token usage from `state.monitor`; update `tera_instance` to include two new templates:

```rust
use axum::response::Html;
use axum::{Router, extract::State, routing::get};
use std::sync::Arc;
use tera::{Context, Tera};
use super::api::AppState;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/panel", get(dashboard))
        .route("/panel/feed/{name}", get(feed_detail))
        .route("/panel/settings", get(settings))
        .route("/panel/monitor", get(monitor_page))
        .route("/panel/feed/{name}/logs", get(feed_logs_page))
        .with_state(state)
}

fn tera_instance() -> Result<Tera, crate::error::AppError> {
    let mut tera = Tera::default();
    tera.add_raw_template("base.html", include_str!("../../templates/base.html"))?;
    tera.add_raw_template("dashboard.html", include_str!("../../templates/dashboard.html"))?;
    tera.add_raw_template("feed.html", include_str!("../../templates/feed.html"))?;
    tera.add_raw_template("settings.html", include_str!("../../templates/settings.html"))?;
    tera.add_raw_template("monitor.html", include_str!("../../templates/monitor.html"))?;
    tera.add_raw_template("logs.html", include_str!("../../templates/logs.html"))?;
    Ok(tera)
}

async fn dashboard(State(s): State<Arc<AppState>>) -> Result<Html<String>, crate::error::AppError> {
    let stats = crate::storage::all_feed_stats()?;
    let cfg = s.config.read().await;
    let mon = s.monitor.read().await;
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", "rssume Dashboard");
    ctx.insert("feeds", &cfg.feeds);
    ctx.insert("stats", &stats);
    ctx.insert("total_prompt_tokens", &mon.token_usage.total_prompt_tokens);
    ctx.insert("total_completion_tokens", &mon.token_usage.total_completion_tokens);
    Ok(Html(tera.render("dashboard.html", &ctx)
        .map_err(|e| crate::error::AppError::Storage(format!("render: {}", e)))?))
}

async fn feed_detail(
    State(s): State<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Html<String>, crate::error::AppError> {
    let data = crate::storage::FeedData::load(&name)?;
    let mon = s.monitor.read().await;
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", &format!("rssume - {}", name));
    ctx.insert("feed_name", &name);
    ctx.insert("articles", &data.articles);
    ctx.insert("runtime_status", &mon.feeds.get(&name).map(|s| &s.status));
    Ok(Html(tera.render("feed.html", &ctx)
        .map_err(|e| crate::error::AppError::Storage(format!("render: {}", e)))?))
}

async fn monitor_page(State(_s): State<Arc<AppState>>) -> Result<Html<String>, crate::error::AppError> {
    let tera = tera_instance()?;
    let mut ctx = Context::new(); ctx.insert("title", "rssume Monitor");
    Ok(Html(tera.render("monitor.html", &ctx)
        .map_err(|e| crate::error::AppError::Storage(format!("render: {}", e)))?))
}

async fn feed_logs_page(
    State(s): State<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Html<String>, crate::error::AppError> {
    let mon = s.monitor.read().await;
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", &format!("rssume - {} logs", name));
    ctx.insert("feed_name", &name);
    ctx.insert("logs", &mon.get_logs(&name));
    Ok(Html(tera.render("logs.html", &ctx)
        .map_err(|e| crate::error::AppError::Storage(format!("render: {}", e)))?))
}

// settings() stays same but loads from disk (no shared config needed for settings)
async fn settings() -> Result<Html<String>, crate::error::AppError> {
    let config = crate::config::Config::load().unwrap_or_else(|_| crate::config::Config {
        server: crate::config::ServerConfig { host: "127.0.0.1".into(), port: 3000 },
        language: crate::config::LanguageConfig { target: "zho".into() },
        llm: crate::config::LlmConfig {
            translation: crate::config::LlmProviderConfig { provider: "".into(), model: "".into(), api_key: "".into(), base_url: "".into(), prompt_append: None },
            summary: crate::config::LlmProviderConfig { provider: "".into(), model: "".into(), api_key: "".into(), base_url: "".into(), prompt_append: None },
        },
        feeds: vec![], logging: Default::default(),
    });
    let tera = tera_instance()?;
    let mut ctx = Context::new(); ctx.insert("title", "rssume Settings"); ctx.insert("config", &config);
    Ok(Html(tera.render("settings.html", &ctx)
        .map_err(|e| crate::error::AppError::Storage(format!("render: {}", e)))?))
}
```

- [ ] **Step 3: Verify** — `cargo check 2>&1` — will fail because templates don't exist yet. Expected.

- [ ] **Step 4: Commit**

---

### Task 10: Create templates — dashboard (updated), monitor.html, logs.html

**Files:** Modify: `templates/dashboard.html`, `templates/base.html`; Create: `templates/monitor.html`, `templates/logs.html`

- [ ] **Step 1: Update `templates/base.html` nav** — change the nav items (lines 216-218):

```html
<nav>
    <a href="/panel" class="{% if title and 'Dashboard' in title %}active{% endif %}">Dashboard</a>
    <a href="/panel/monitor" class="{% if title and 'Monitor' in title %}active{% endif %}">Monitor</a>
    <a href="/panel/settings" class="{% if title and 'Settings' in title %}active{% endif %}">Settings</a>
</nav>
```

- [ ] **Step 2: Update `templates/dashboard.html`** — add 5th stat card (total tokens), add `hx-get` and `hx-trigger` to stats-bar and table:

```html
{% extends "base.html" %}
{% block title %}rssume Dashboard{% endblock %}
{% block content %}
<h1>Dashboard</h1>
<p class="subtitle">Monitor your RSS feeds and AI processing status</p>

<div class="stats-bar" hx-get="/api/stats" hx-trigger="every 30s" hx-swap="outerHTML">
    <div class="stat-card">
        <div class="number">{{ feeds | length }}</div><div class="label">Feeds</div>
    </div>
    <div class="stat-card">
        <div class="number">{% set t=0 %}{% for s in stats %}{% set t=t+s.article_count %}{% endfor %}{{ t }}</div>
        <div class="label">Articles</div>
    </div>
    <div class="stat-card">
        <div class="number">{% set t=0 %}{% for s in stats %}{% set t=t+s.translated_count %}{% endfor %}{{ t }}</div>
        <div class="label">Translated</div>
    </div>
    <div class="stat-card">
        <div class="number">{% set t=0 %}{% for s in stats %}{% set t=t+s.with_summary_count %}{% endfor %}{{ t }}</div>
        <div class="label">Summarized</div>
    </div>
    <div class="stat-card">
        <div class="number">{{ total_prompt_tokens + total_completion_tokens }}</div>
        <div class="label">Tokens</div>
    </div>
</div>

<h2>Feeds</h2>
<table hx-get="/api/monitor/status" hx-trigger="every 15s" hx-swap="outerHTML">
    <thead><tr><th>Name</th><th>Status</th><th>Articles</th><th>Translated</th><th>Summarized</th><th>Last Fetch</th><th>RSS</th></tr></thead>
    <tbody>
        {% for feed in feeds %}
        <tr>
            <td><a href="/panel/feed/{{ feed.name }}" style="color:var(--link);text-decoration:none;">{{ feed.name }}</a></td>
            <td><span class="status-dot" id="status-{{ feed.name }}">●</span></td>
            <td>{% set c=0 %}{% for s in stats %}{% if s.feed_name==feed.name %}{% set c=s.article_count %}{% endif %}{% endfor %}{{ c }}</td>
            <td>{% set c=0 %}{% for s in stats %}{% if s.feed_name==feed.name %}{% set c=s.translated_count %}{% endif %}{% endfor %}<span class="badge {% if c>0 %}badge-success{% endif %}">{{ c }}</span></td>
            <td>{% set c=0 %}{% for s in stats %}{% if s.feed_name==feed.name %}{% set c=s.with_summary_count %}{% endif %}{% endfor %}<span class="badge {% if c>0 %}badge-success{% endif %}">{{ c }}</span></td>
            <td style="font-size:12px;color:var(--mute);">--</td>
            <td><code style="font-size:12px;">/feeds/{{ feed.name }}</code></td>
        </tr>
        {% endfor %}
    </tbody>
</table>

{% if feeds | length == 0 %}
<div class="empty-state"><h3>No feeds configured</h3><p>Add RSS feeds to your config.toml to get started.</p></div>
{% endif %}
{% endblock %}
```

- [ ] **Step 3: Create `templates/monitor.html`**

```html
{% extends "base.html" %}
{% block title %}rssume Monitor{% endblock %}
{% block content %}
<h1>Live Monitor</h1>
<p class="subtitle">Real-time translation progress and feed status</p>

<div hx-get="/api/monitor/translating" hx-trigger="every 2s" hx-swap="outerHTML">
    <div class="grid-2" style="margin-bottom:24px;">
        {% for feed in feeds %}
        <div class="card">
            <div style="display:flex;align-items:center;gap:8px;margin-bottom:8px;">
                <span class="status-dot">●</span>
                <strong>{{ feed.name }}</strong>
            </div>
            <div style="font-size:13px;color:var(--mute);">Loading...</div>
        </div>
        {% endfor %}
    </div>

    <h2>Active Translations</h2>
    <div id="active-translations">
        <p style="color:var(--mute);">Waiting for translation activity...</p>
    </div>

    <h2 style="margin-top:24px;">Recently Completed</h2>
    <div id="recent-logs">
        <p style="color:var(--mute);">No recent translations</p>
    </div>
</div>
{% endblock %}
```

- [ ] **Step 4: Create `templates/logs.html`**

```html
{% extends "base.html" %}
{% block title %}rssume - {{ feed_name }} logs{% endblock %}
{% block content %}
<h1>{{ feed_name }} — Translation Logs</h1>
<p class="subtitle">{{ logs | length }} entries (max 500, newest first)</p>

<div style="margin-bottom:16px;">
    <a href="/panel" class="btn">← Dashboard</a>
    <a href="/panel/feed/{{ feed_name }}" class="btn" style="margin-left:8px;">Feed Detail</a>
    <a href="/panel/monitor" class="btn" style="margin-left:8px;">Live Monitor</a>
</div>

{% for log in logs | reverse %}
<div class="card" style="margin-bottom:8px;padding:16px;">
    <div style="display:flex;justify-content:space-between;align-items:center;">
        <div>
            <strong style="font-size:14px;">{{ log.article_title | truncate(length=80) }}</strong>
            <div style="display:flex;gap:6px;margin-top:4px;flex-wrap:wrap;">
                <span class="badge">{{ log.stage }}</span>
                <span class="badge">{% if log.status == "Completed" %}badge-success{% elif log.status is string and "Failed" in log.status %}badge-warning{% else %}badge{% endif %}">{{ log.status }}</span>
                <span class="badge">{{ log.model }}</span>
            </div>
        </div>
        <div style="text-align:right;font-size:12px;color:var(--mute);">
            <div>{{ log.timestamp | truncate(length=19) }}</div>
            {% if log.prompt_tokens %}
            <div style="margin-top:2px;">in: {{ log.prompt_tokens }} · out: {{ log.completion_tokens }}</div>
            {% endif %}
        </div>
    </div>
</div>
{% endfor %}

{% if logs | length == 0 %}
<div class="empty-state"><h3>No logs yet</h3><p>Translation logs will appear once feed polling starts.</p></div>
{% endif %}
{% endblock %}
```

- [ ] **Step 5: Verify** — `cargo check 2>&1` — should compile.

- [ ] **Step 6: Commit**

---

### Task 11: Wire everything in main.rs

**Files:** `src/main.rs`

- [ ] **Step 1: Update main.rs** — create Monitor, pass to Scheduler and web router:

```rust
mod config;
mod error;
mod lang;
mod llm;
mod monitor;
mod rss;
mod scheduler;
mod storage;
mod web;

use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = match config::Config::load() {
        Ok(c) => { tracing::info!("Config loaded from {}", config::config_path().display()); c }
        Err(e) => { tracing::error!("Failed to load config: {}", e); std::process::exit(1); }
    };

    let data_dir = config::Config::data_dir();
    std::fs::create_dir_all(&data_dir).ok();
    tracing::info!("Data directory: {}", data_dir.display());

    let config = Arc::new(RwLock::new(config));
    let monitor = Arc::new(RwLock::new(monitor::Monitor::new()));

    let scheduler = Arc::new(scheduler::Scheduler::new(config.clone(), monitor.clone()));
    let s = scheduler.clone();
    tokio::spawn(async move { s.run_loop().await; });

    let app = web::router(config.clone(), monitor.clone());

    let host = config.read().await.server.host.clone();
    let port = config.read().await.server.port;
    let addr = format!("{}:{}", host, port);

    tracing::info!("rssume starting on http://{}", addr);
    tracing::info!("  Web panel:  http://{}/panel", addr);
    tracing::info!("  Monitor:    http://{}/panel/monitor", addr);
    tracing::info!("  RSS feeds:  http://{}/feeds/{{feed_name}}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl+c");
    tracing::info!("Shutting down...");
}
```

- [ ] **Step 2: Verify** — `cargo check 2>&1` — should pass.

- [ ] **Step 3: Build and run a quick smoke test** — `cargo build --release 2>&1` should succeed.

- [ ] **Step 4: Commit**

---

### Task 12: Final verification — build + clippy + test

**Files:** none

- [ ] **Step 1: Full build** — `cargo build --release 2>&1` — must succeed
- [ ] **Step 2: Clippy** — `cargo clippy -- -D warnings 2>&1` — fix any warnings
- [ ] **Step 3: Tests** — `cargo test 2>&1` — all existing tests pass
- [ ] **Step 4: Run dev** — `cargo run 2>&1` — starts without errors, visit http://localhost:3000/panel
- [ ] **Step 5: Commit any final fixes**
