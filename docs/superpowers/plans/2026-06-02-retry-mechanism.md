# 重试机制完善实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 LLM 翻译的重试机制从硬编码改为可配置，支持重试时清理失败状态并创建新日志。

**Architecture:** 在 LlmConfig 中添加重试配置，创建 RetryContext 结构体重试状态管理，修改 translate_and_summarize 函数使用新的重试逻辑。

**Tech Stack:** Rust, serde, tokio, tracing

---

## 文件结构

```
src/
├── config.rs                    # 添加重试配置字段
├── llm/
│   ├── mod.rs                   # 添加 RetryContext 结构体
│   └── translate_summarize.rs   # 修改函数签名和重试逻辑
└── scheduler.rs                 # 修改调用方式
```

---

## Task 1: 添加重试配置字段

**Files:**
- Modify: `src/config.rs:28-34`
- Test: `src/config.rs` (内联测试)

- [ ] **Step 1: 在 LlmConfig 中添加重试配置字段**

在 `src/config.rs` 的 `LlmConfig` 结构体中添加 `max_retries` 和 `retry_delay_secs` 字段：

```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LlmConfig {
    pub translation: LlmProviderConfig,
    pub summary: LlmProviderConfig,
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_delay_secs")]
    pub retry_delay_secs: u64,
}
```

- [ ] **Step 2: 添加默认值函数**

在 `src/config.rs` 的默认值函数区域添加：

```rust
fn default_max_retries() -> u32 {
    2
}

fn default_retry_delay_secs() -> u64 {
    1
}
```

- [ ] **Step 3: 更新默认配置模板**

在 `src/config.rs` 的 `default_config_toml()` 函数中，在 `[llm]` 部分添加：

```rust
max_retries = 2
retry_delay_secs = 1
```

- [ ] **Step 4: 验证配置解析**

运行以下命令验证配置解析正常：

```bash
cargo test --lib config
```

预期：测试通过，无编译错误

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: add retry configuration fields to LlmConfig"
```

---

## Task 2: 创建 RetryContext 结构体

**Files:**
- Create: `src/llm/retry.rs`
- Modify: `src/llm/mod.rs`
- Test: `src/llm/retry.rs` (内联测试)

- [ ] **Step 1: 创建 retry.rs 文件**

创建 `src/llm/retry.rs` 文件，定义 RetryContext 结构体：

```rust
use crate::error::AppError;
use crate::monitor::{LogStatus, Monitor, TranslationLog, TranslationStage};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct RetryContext {
    pub max_retries: u32,
    pub retry_delay_secs: u64,
    pub current_attempt: u32,
    pub last_error: Option<AppError>,
    pub feed_name: String,
    pub article_title: String,
    pub model: String,
    pub monitor: Arc<RwLock<Monitor>>,
    pub current_log_id: Option<String>,
}

impl RetryContext {
    pub fn new(
        max_retries: u32,
        retry_delay_secs: u64,
        feed_name: String,
        article_title: String,
        model: String,
        monitor: Arc<RwLock<Monitor>>,
    ) -> Self {
        RetryContext {
            max_retries,
            retry_delay_secs,
            current_attempt: 0,
            last_error: None,
            feed_name,
            article_title,
            model,
            monitor,
            current_log_id: None,
        }
    }

    pub fn should_retry(&self) -> bool {
        self.current_attempt < self.max_retries
    }

    pub async fn record_failure(&mut self, error: AppError) {
        self.last_error = Some(error);
        if let Some(log_id) = &self.current_log_id {
            let error_msg = self.last_error.as_ref().map(|e| e.to_string()).unwrap_or_default();
            self.monitor.write().await.update_log(&self.feed_name, log_id, |log| {
                log.status = LogStatus::Failed(error_msg);
            });
        }
    }

    pub async fn prepare_retry(&mut self) {
        self.current_attempt += 1;
        let log = TranslationLog {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            article_title: self.article_title.clone(),
            stage: TranslationStage::TranslateAndSummarize,
            status: LogStatus::Started,
            model: self.model.clone(),
            prompt_tokens: None,
            completion_tokens: None,
            streamed_text: String::new(),
        };
        self.current_log_id = Some(log.id.clone());
        self.monitor.write().await.add_log(&self.feed_name, log);
    }

    pub async fn wait(&self) {
        if self.retry_delay_secs > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(self.retry_delay_secs)).await;
        }
    }

    pub async fn mark_success(&self, usage: &crate::llm::UsageInfo) {
        if let Some(log_id) = &self.current_log_id {
            self.monitor.write().await.update_log(&self.feed_name, log_id, |log| {
                log.status = LogStatus::Completed;
                log.prompt_tokens = Some(usage.prompt_tokens);
                log.completion_tokens = Some(usage.completion_tokens);
            });
            self.monitor.write().await.add_token_usage(
                &self.feed_name,
                &self.model,
                usage.prompt_tokens,
                usage.completion_tokens,
            );
        }
    }
}
```

- [ ] **Step 2: 在 mod.rs 中声明 retry 模块**

在 `src/llm/mod.rs` 中添加：

```rust
pub mod retry;
pub mod translate_summarize;
```

- [ ] **Step 3: 验证编译**

运行以下命令验证编译通过：

```bash
cargo check
```

预期：无编译错误

- [ ] **Step 4: Commit**

```bash
git add src/llm/retry.rs src/llm/mod.rs
git commit -m "feat: add RetryContext struct for retry state management"
```

---

## Task 3: 修改 translate_and_summarize 函数

**Files:**
- Modify: `src/llm/translate_summarize.rs`
- Test: `src/llm/translate_summarize.rs` (内联测试)

- [ ] **Step 1: 修改函数签名**

修改 `translate_and_summarize` 函数签名，使用 `RetryContext` 替代原有的 `on_token` 参数：

```rust
pub async fn translate_and_summarize(
    config: &LlmProviderConfig,
    title: &str,
    content: &str,
    target_lang: &str,
    retry_ctx: &mut super::retry::RetryContext,
) -> Result<(StreamResult, ParsedArticle), crate::error::AppError> {
```

- [ ] **Step 2: 重写重试循环**

替换原有的重试循环逻辑：

```rust
let prompt = format!(
    "Target language: {}\n\nTitle: {}\n\nContent:\n{}",
    target_lang, title, content
);
let append = config.prompt_append.clone().unwrap_or_default();
let full = if append.is_empty() {
    prompt
} else {
    format!("{}\n{}", prompt, append)
};

loop {
    retry_ctx.prepare_retry().await;
    let log_id = retry_ctx.current_log_id.clone().unwrap();
    let monitor = retry_ctx.monitor.clone();
    let feed_name = retry_ctx.feed_name.clone();

    let ot = move |t: &str| {
        let m = monitor.clone();
        let f = feed_name.clone();
        let l = log_id.clone();
        let s = t.to_string();
        tokio::task::spawn(async move {
            m.write().await.update_log(&f, &l, |log| {
                log.streamed_text.push_str(&s);
                log.status = LogStatus::Streaming {
                    tokens: log.streamed_text.clone(),
                };
            });
        });
    };

    match chat_stream(config, SYSTEM_PROMPT, &full, ot).await {
        Ok(result) => {
            match parse_llm_output(&result.text) {
                Ok(parsed) => {
                    retry_ctx.mark_success(&result.usage).await;
                    return Ok((result, parsed));
                }
                Err(e) => {
                    retry_ctx.record_failure(e).await;
                    if !retry_ctx.should_retry() {
                        return Err(retry_ctx.last_error.take().unwrap());
                    }
                    retry_ctx.wait().await;
                }
            }
        }
        Err(e) => {
            retry_ctx.record_failure(e).await;
            if !retry_ctx.should_retry() {
                return Err(retry_ctx.last_error.take().unwrap());
            }
            retry_ctx.wait().await;
        }
    }
}
```

- [ ] **Step 3: 删除旧的重试相关代码**

删除原有的 `MAX_RETRIES` 常量和 `mtok` 函数（如果存在）。

- [ ] **Step 4: 验证编译**

运行以下命令验证编译通过：

```bash
cargo check
```

预期：无编译错误

- [ ] **Step 5: Commit**

```bash
git add src/llm/translate_summarize.rs
git commit -m "feat: refactor translate_and_summarize to use RetryContext"
```

---

## Task 4: 修改 scheduler.rs 调用方式

**Files:**
- Modify: `src/scheduler.rs:282-413`
- Test: `src/scheduler.rs` (内联测试)

- [ ] **Step 1: 添加 retry 模块引用**

在 `src/scheduler.rs` 顶部添加：

```rust
use crate::llm::retry::RetryContext;
```

- [ ] **Step 2: 修改 process_single_article 函数**

在 `process_single_article` 函数中，创建 `RetryContext` 并传递给 `translate_and_summarize`：

```rust
let model = tc.model.clone();

let _permit = semaphore.acquire().await.unwrap();

let mut retry_ctx = RetryContext::new(
    config.llm.max_retries,
    config.llm.retry_delay_secs,
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
```

- [ ] **Step 3: 删除旧的日志创建代码**

删除原有的 `mlog` 函数调用和 `mtok` 函数定义（如果存在）。

- [ ] **Step 4: 验证编译**

运行以下命令验证编译通过：

```bash
cargo check
```

预期：无编译错误

- [ ] **Step 5: Commit**

```bash
git add src/scheduler.rs
git commit -m "feat: update scheduler to use RetryContext for retry management"
```

---

## Task 5: 集成测试和验证

**Files:**
- Test: 整体功能验证

- [ ] **Step 1: 运行所有测试**

```bash
cargo test
```

预期：所有测试通过

- [ ] **Step 2: 运行 clippy 检查**

```bash
cargo clippy --all-targets
```

预期：无警告

- [ ] **Step 3: 验证配置解析**

创建测试配置文件验证新配置字段：

```bash
cat > /tmp/test_config.toml << 'EOF'
[server]
host = "127.0.0.1"
port = 3000

[language]
target = "zh_CN"

[llm]
max_concurrent_requests = 3
max_retries = 3
retry_delay_secs = 2

[llm.translation]
provider = "openai"
model = "gpt-4o-mini"
api_key = "test"
base_url = "https://api.openai.com/v1"

[llm.summary]
provider = "openai"
model = "gpt-4o-mini"
api_key = "test"
base_url = "https://api.openai.com/v1"

[[feeds]]
name = "test"
url = "https://example.com/feed.xml"
enabled = true
interval_secs = 300
max_articles = 25

[logging]
level = "info"
EOF
```

- [ ] **Step 4: 提交最终版本**

```bash
git add -A
git commit -m "feat: complete retry mechanism with configurable retries and context cleanup"
```

---

## 自检清单

- [x] **规范覆盖：** 所有规范要求都已覆盖
  - [x] 重试次数可配置
  - [x] 重试延迟可配置
  - [x] 重试时清理失败状态
  - [x] 保留失败日志，创建新日志

- [x] **占位符扫描：** 无 "TBD", "TODO" 或不完整部分

- [x] **类型一致性：** 所有类型、方法签名和属性名在任务间保持一致
