# 并发翻译支持实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 rssume 支持并发处理多个 Feed 和多篇文章，提升翻译吞吐量，同时通过可配置的并发限制防止 LLM API 限流。

**Architecture:** 使用 `tokio::sync::Semaphore` 控制并发数，Feed 间和文章间共享同一个信号量。`FeedStatus::Translating` 结构改为追踪 `completed` 和 `in_progress` 列表，适配并发场景。

**Tech Stack:** tokio (Semaphore, join_all), serde, Tera templates

---

### Task 1: 添加并发配置

**Files:**
- Modify: `src/config.rs:29-32`

- [ ] **Step 1: 在 LlmConfig 中添加 max_concurrent_requests 字段**

将：
```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LlmConfig {
    pub translation: LlmProviderConfig,
    pub summary: LlmProviderConfig,
}
```

改为：
```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LlmConfig {
    pub translation: LlmProviderConfig,
    pub summary: LlmProviderConfig,
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
}
```

- [ ] **Step 2: 添加默认值函数**

在 `src/config.rs` 的默认值函数区域（约 63-83 行）添加：
```rust
fn default_max_concurrent_requests() -> usize {
    3
}
```

- [ ] **Step 3: 更新默认配置模板**

将 `default_config_toml()` 函数中的：
```rust
[llm.translation]
```

改为：
```rust
[llm]
max_concurrent_requests = 3

[llm.translation]
```

- [ ] **Step 4: 验证编译**

Run: `cargo build --release`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: add max_concurrent_requests config for translation concurrency"
```

---

### Task 2: 修改 Monitor 状态结构

**Files:**
- Modify: `src/monitor.rs:122-135`

- [ ] **Step 1: 修改 FeedStatus::Translating 结构**

将：
```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum FeedStatus {
    Idle,
    Fetching,
    Translating {
        current: u32,
        total: u32,
        current_title: String,
    },
    Done,
    #[allow(dead_code)]
    Error(String),
}
```

改为：
```rust
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
```

- [ ] **Step 2: 添加辅助方法**

在 `Monitor` impl 块中添加：
```rust
pub fn start_article(&mut self, feed_name: &str, title: &str, total: u32) {
    self.feeds.entry(feed_name.to_string()).and_modify(|s| {
        if let FeedStatus::Translating { ref mut in_progress, .. } = s.status {
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
        if let FeedStatus::Translating { ref mut completed, ref mut in_progress, .. } = s.status {
            *completed += 1;
            in_progress.retain(|t| t != title);
        }
    });
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo build --release`
Expected: 编译失败（scheduler.rs 和 web 模块引用了旧结构）

- [ ] **Step 4: Commit**

```bash
git add src/monitor.rs
git commit -m "refactor: update FeedStatus::Translating for concurrent tracking"
```

---

### Task 3: 重构 Scheduler 为并发处理

**Files:**
- Modify: `src/scheduler.rs`

- [ ] **Step 1: 添加必要的 import**

将：
```rust
use crate::config::Config;
use crate::llm::{summarize, translate};
use crate::monitor::{FeedStatus, LogStatus, Monitor, TranslationLog, TranslationStage};
use crate::rss::fetch;
use crate::storage::{Article, Enclosure, FeedData};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
```

改为：
```rust
use crate::config::Config;
use crate::llm::{summarize, translate};
use crate::monitor::{FeedStatus, LogStatus, Monitor, TranslationLog, TranslationStage};
use crate::rss::fetch;
use crate::storage::{Article, Enclosure, FeedData};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;
```

- [ ] **Step 2: 在 Scheduler 中添加信号量**

将：
```rust
pub struct Scheduler {
    config: Arc<RwLock<Config>>,
    monitor: Arc<RwLock<Monitor>>,
}

impl Scheduler {
    pub fn new(config: Arc<RwLock<Config>>, monitor: Arc<RwLock<Monitor>>) -> Self {
        Scheduler { config, monitor }
    }
```

改为：
```rust
pub struct Scheduler {
    config: Arc<RwLock<Config>>,
    monitor: Arc<RwLock<Monitor>>,
    semaphore: Arc<Semaphore>,
}

impl Scheduler {
    pub fn new(config: Arc<RwLock<Config>>, monitor: Arc<RwLock<Monitor>>) -> Self {
        let max_concurrent = config.try_read().map(|c| c.llm.max_concurrent_requests).unwrap_or(3);
        Scheduler {
            config,
            monitor,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }
```

- [ ] **Step 3: 重构 process_feed 为并发处理**

将整个 `process_feed` 方法替换为：
```rust
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

    // Set initial translating status
    self.monitor.write().await.set_status(
        feed_name,
        FeedStatus::Translating {
            completed: 0,
            total,
            in_progress: vec![],
        },
    );

    let tc = config.llm.translation.clone();
    let sc = config.llm.summary.clone();
    let target = config.language.target.clone();
    let semaphore = self.semaphore.clone();
    let monitor = self.monitor.clone();
    let feed_name_owned = feed_name.to_string();

    // Process articles concurrently
    let handles: Vec<_> = new_articles
        .into_iter()
        .map(|raw| {
            let tc = tc.clone();
            let sc = sc.clone();
            let target = target.clone();
            let semaphore = semaphore.clone();
            let monitor = monitor.clone();
            let feed_name = feed_name_owned.clone();

            tokio::spawn(async move {
                let result = process_single_article(
                    &feed_name,
                    raw,
                    &tc,
                    &sc,
                    &target,
                    semaphore,
                    monitor.clone(),
                )
                .await;
                (feed_name, result)
            })
        })
        .collect();

    // Collect results
    for handle in handles {
        match handle.await {
            Ok((feed_name, Ok(article))) => {
                feed_data.articles.push(article);
                monitor.write().await.complete_article(&feed_name, &article.original_title);
            }
            Ok((feed_name, Err(e))) => {
                tracing::error!("Article processing failed '{}': {}", feed_name, e);
            }
            Err(e) => {
                tracing::error!("Task join error: {}", e);
            }
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
```

- [ ] **Step 4: 重构 process_all 为并发处理**

将：
```rust
pub async fn process_all(&self) {
    let cfg = self.config.read().await;
    for f in &cfg.feeds {
        if f.enabled {
            self.process_feed(&f.name, &f.url).await;
        }
    }
}
```

改为：
```rust
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
            let scheduler = self.clone_handle();
            tokio::spawn(async move {
                scheduler.process_feed(&name, &url).await;
            })
        })
        .collect();

    for handle in handles {
        if let Err(e) = handle.await {
            tracing::error!("Feed task join error: {}", e);
        }
    }
}
```

- [ ] **Step 5: 添加 clone_handle 辅助方法**

在 `Scheduler` impl 块中添加：
```rust
fn clone_handle(&self) -> Self {
    Scheduler {
        config: self.config.clone(),
        monitor: self.monitor.clone(),
        semaphore: self.semaphore.clone(),
    }
}
```

- [ ] **Step 6: 添加 process_single_article 函数**

在文件末尾（`mtok` 函数之后）添加：
```rust
async fn process_single_article(
    feed_name: &str,
    raw: crate::rss::fetch::RawArticle,
    tc: &crate::config::LlmProviderConfig,
    sc: &crate::config::LlmProviderConfig,
    target: &str,
    semaphore: Arc<Semaphore>,
    monitor: Arc<RwLock<Monitor>>,
) -> Result<Article, crate::error::AppError> {
    let title = raw.title.clone();
    monitor.write().await.start_article(feed_name, &title, 1);

    let source_lang =
        crate::lang::detect(&raw.content).or_else(|| crate::lang::detect(&raw.title));
    let needs_ct =
        !raw.content.is_empty() && crate::lang::needs_translation(&raw.content, target);
    let needs_tt = crate::lang::needs_translation(&raw.title, target);
    let model = tc.model.clone();
    let sum_model = sc.model.clone();
    let mut total_translation_tokens: u32 = 0;

    // ---- Title Translation ----
    let (final_title, tt) = if needs_tt {
        let _permit = semaphore.acquire().await.unwrap();
        let log = mlog(&raw.title, TranslationStage::TranslatingTitle, &model);
        let lid = log.id.clone();
        monitor.write().await.add_log(feed_name, log);
        let ot = mtok(monitor.clone(), feed_name.to_string(), lid.clone());
        match translate::translate(tc, &raw.title, target, ot).await {
            Ok(r) => {
                let translated = r.text != raw.title;
                monitor.write().await.update_log(feed_name, &lid, |l| {
                    if translated {
                        l.status = LogStatus::Completed;
                        l.prompt_tokens = Some(r.usage.prompt_tokens);
                        l.completion_tokens = Some(r.usage.completion_tokens);
                    } else {
                        l.status = LogStatus::Failed("model returned untranslated text".into());
                    }
                });
                if translated {
                    total_translation_tokens +=
                        r.usage.prompt_tokens + r.usage.completion_tokens;
                    monitor.write().await.add_token_usage(
                        feed_name,
                        &model,
                        r.usage.prompt_tokens,
                        r.usage.completion_tokens,
                    );
                }
                (
                    if translated { r.text } else { raw.title.clone() },
                    translated,
                )
            }
            Err(e) => {
                monitor.write().await.update_log(feed_name, &lid, |l| {
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
        let _permit = semaphore.acquire().await.unwrap();
        let log = mlog(&raw.title, TranslationStage::TranslatingContent, &model);
        let lid = log.id.clone();
        monitor.write().await.add_log(feed_name, log);
        let ot = mtok(monitor.clone(), feed_name.to_string(), lid.clone());
        match translate::translate(tc, &raw.content, target, ot).await {
            Ok(r) => {
                let translated = r.text != raw.content;
                monitor.write().await.update_log(feed_name, &lid, |l| {
                    if translated {
                        l.status = LogStatus::Completed;
                        l.prompt_tokens = Some(r.usage.prompt_tokens);
                        l.completion_tokens = Some(r.usage.completion_tokens);
                    } else {
                        l.status = LogStatus::Failed("model returned untranslated text".into());
                    }
                });
                if translated {
                    total_translation_tokens +=
                        r.usage.prompt_tokens + r.usage.completion_tokens;
                    monitor.write().await.add_token_usage(
                        feed_name,
                        &model,
                        r.usage.prompt_tokens,
                        r.usage.completion_tokens,
                    );
                }
                (
                    if translated { r.text } else { raw.content.clone() },
                    translated,
                )
            }
            Err(e) => {
                monitor.write().await.update_log(feed_name, &lid, |l| {
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
        let _permit = semaphore.acquire().await.unwrap();
        let log = mlog(&final_title, TranslationStage::Summarizing, &sum_model);
        let lid = log.id.clone();
        monitor.write().await.add_log(feed_name, log);
        let ot = mtok(monitor.clone(), feed_name.to_string(), lid.clone());
        match summarize::summarize(sc, &final_title, &final_content, ot).await {
            Ok(r) => {
                monitor.write().await.update_log(feed_name, &lid, |l| {
                    l.status = LogStatus::Completed;
                    l.prompt_tokens = Some(r.usage.prompt_tokens);
                    l.completion_tokens = Some(r.usage.completion_tokens);
                });
                monitor.write().await.add_token_usage(
                    feed_name,
                    &sum_model,
                    r.usage.prompt_tokens,
                    r.usage.completion_tokens,
                );
                Some(r.text)
            }
            Err(e) => {
                monitor.write().await.update_log(feed_name, &lid, |l| {
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
    })
}
```

- [ ] **Step 7: 验证编译**

Run: `cargo build --release`
Expected: 编译失败（web 模块引用了旧的 FeedStatus 结构）

- [ ] **Step 8: Commit**

```bash
git add src/scheduler.rs
git commit -m "feat: implement concurrent feed and article processing"
```

---

### Task 4: 更新 Web API 模块

**Files:**
- Modify: `src/web/api.rs:121-146, 209-228`

- [ ] **Step 1: 更新 monitor_translating 中的 FeedStatus 匹配**

将：
```rust
"translating_current": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { current, .. }) => current,
    _ => &0u32,
},
"translating_total": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
    _ => &0u32,
},
"translating_title": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { current_title, .. }) => current_title,
    _ => "",
},
```

改为：
```rust
"translating_completed": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { completed, .. }) => completed,
    _ => &0u32,
},
"translating_total": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
    _ => &0u32,
},
"translating_in_progress": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { in_progress, .. }) => in_progress.clone(),
    _ => vec![],
},
```

（在 `monitor_translating` 函数中约 132-144 行）

- [ ] **Step 2: 更新 monitor_feed_translating 中的 FeedStatus 匹配**

将：
```rust
"translating_current": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { current, .. }) => current,
    _ => &0u32,
},
"translating_total": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
    _ => &0u32,
},
"translating_title": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { current_title, .. }) => current_title,
    _ => "",
},
```

改为：
```rust
"translating_completed": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { completed, .. }) => completed,
    _ => &0u32,
},
"translating_total": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
    _ => &0u32,
},
"translating_in_progress": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { in_progress, .. }) => in_progress.clone(),
    _ => vec![],
},
```

（在 `monitor_feed_translating` 函数中约 216-228 行）

- [ ] **Step 3: 验证编译**

Run: `cargo build --release`
Expected: 编译失败（panel.rs 也需要更新）

- [ ] **Step 4: Commit**

```bash
git add src/web/api.rs
git commit -m "refactor: update api.rs for new FeedStatus structure"
```

---

### Task 5: 更新 Web Panel 模块

**Files:**
- Modify: `src/web/panel.rs:146-158, 244-256`

- [ ] **Step 1: 更新 monitor_page 中的 FeedStatus 匹配**

将：
```rust
"translating_current": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { current, .. }) => current,
    _ => &0u32,
},
"translating_total": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
    _ => &0u32,
},
"translating_title": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { current_title, .. }) => current_title,
    _ => "",
},
```

改为：
```rust
"translating_completed": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { completed, .. }) => completed,
    _ => &0u32,
},
"translating_total": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
    _ => &0u32,
},
"translating_in_progress": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { in_progress, .. }) => in_progress.clone(),
    _ => vec![],
},
```

（在 `monitor_page` 函数中约 146-158 行）

- [ ] **Step 2: 更新 feed_monitor_page 中的 FeedStatus 匹配**

将：
```rust
"translating_current": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { current, .. }) => current,
    _ => &0u32,
},
"translating_total": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
    _ => &0u32,
},
"translating_title": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { current_title, .. }) => current_title,
    _ => "",
},
```

改为：
```rust
"translating_completed": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { completed, .. }) => completed,
    _ => &0u32,
},
"translating_total": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
    _ => &0u32,
},
"translating_in_progress": match rt.map(|r| &r.status) {
    Some(crate::monitor::FeedStatus::Translating { in_progress, .. }) => in_progress.clone(),
    _ => vec![],
},
```

（在 `feed_monitor_page` 函数中约 244-256 行）

- [ ] **Step 3: 验证编译**

Run: `cargo build --release`
Expected: 编译失败（模板引用了旧变量名）

- [ ] **Step 4: Commit**

```bash
git add src/web/panel.rs
git commit -m "refactor: update panel.rs for new FeedStatus structure"
```

---

### Task 6: 更新模板

**Files:**
- Modify: `templates/partials/feed_monitor_status.html:9-11`
- Modify: `templates/partials/monitor_status.html:10-11`

- [ ] **Step 1: 更新 feed_monitor_status.html**

将：
```html
{% if feed.status is starting_with('Translating') %}
<span>Translating {{ feed.translating_current }}/{{ feed.translating_total }}: {{ feed.translating_title | truncate(length=40) }}</span>
{% elif feed.status == 'Fetching' %}
```

改为：
```html
{% if feed.status is starting_with('Translating') %}
<span>Translating {{ feed.translating_completed }}/{{ feed.translating_total }} ({{ feed.translating_in_progress | length }} in progress)</span>
{% elif feed.status == 'Fetching' %}
```

- [ ] **Step 2: 更新 monitor_status.html**

将：
```html
{% if feed.status is starting_with('Translating') %}
    Translating {{ feed.translating_current }}/{{ feed.translating_total }}: {{ feed.translating_title | truncate(length=40) }}
{% elif feed.status == 'Fetching' %}
```

改为：
```html
{% if feed.status is starting_with('Translating') %}
    Translating {{ feed.translating_completed }}/{{ feed.translating_total }} ({{ feed.translating_in_progress | length }} in progress)
{% elif feed.status == 'Fetching' %}
```

- [ ] **Step 3: 验证编译**

Run: `cargo build --release`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add templates/partials/feed_monitor_status.html templates/partials/monitor_status.html
git commit -m "feat: update templates for concurrent translation progress display"
```

---

### Task 7: 集成测试

**Files:**
- None (manual verification)

- [ ] **Step 1: 配置高并发数**

编辑 `~/.config/rssume/config.toml`，设置：
```toml
[llm]
max_concurrent_requests = 5
```

- [ ] **Step 2: 启动服务**

Run: `cargo run --release`
Expected: 服务正常启动

- [ ] **Step 3: 触发翻译**

等待或手动触发 Feed 抓取，观察日志输出

- [ ] **Step 4: 验证 Monitor 页面**

打开 `/panel/monitor`，确认：
- 多个 Feed 同时显示 "Translating X/Y (Z in progress)"
- 并发数不超过配置值

- [ ] **Step 5: 验证翻译结果**

打开 `/panel/feed/:name`，确认所有文章正确翻译和存储

- [ ] **Step 6: Commit（如果需要修复）**

```bash
git add -A
git commit -m "fix: address integration test findings"
```
