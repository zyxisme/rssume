# 重试机制完善设计

**日期：** 2026-06-02
**状态：** 已批准
**作者：** Claude + 用户协作

## 背景

当前 LLM 翻译和摘要功能使用硬编码的重试次数（`MAX_RETRIES = 2`），无法根据实际需求调整。需要将重试机制改为可配置，并在重试时清理失败状态，模拟处理新文章的流程。

## 设计目标

1. 重试次数可配置（全局配置）
2. 重试延迟可配置（固定延迟）
3. 重试时清理失败状态，模拟新文章处理
4. 保留失败日志，创建新日志记录重试

## 详细设计

### 1. 配置结构

在 `LlmConfig` 中添加重试相关字段：

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

默认值：
- `max_retries = 2`
- `retry_delay_secs = 1`

config.toml 示例：
```toml
[llm]
max_concurrent_requests = 3
max_retries = 2
retry_delay_secs = 1
```

### 2. 重试上下文

创建 `RetryContext` 结构体管理重试状态：

```rust
pub struct RetryContext {
    pub max_retries: u32,
    pub retry_delay_secs: u64,
    pub current_attempt: u32,
    pub last_error: Option<crate::error::AppError>,
    pub feed_name: String,
    pub article_title: String,
    pub model: String,
    pub monitor: Arc<RwLock<Monitor>>,
    pub current_log_id: Option<String>,
}
```

关键方法：
- `new()` — 从配置初始化
- `should_retry()` — 判断是否还能重试
- `record_failure()` — 记录失败，标记当前日志为 Failed
- `prepare_retry()` — 创建新日志条目，返回新 log_id
- `wait()` — 执行固定延迟
- `mark_success()` — 标记成功，记录 token 使用量

### 3. 日志处理流程

#### 首次尝试
1. 创建日志条目（状态：`Started`）
2. 设置 `current_log_id`
3. 流式更新 `streamed_text`

#### 失败时
1. 调用 `record_failure()`：将当前日志标记为 `Failed`，记录错误信息
2. 调用 `should_retry()` 检查是否还能重试
3. 如果能重试：
   - 调用 `wait()` 执行延迟
   - 调用 `prepare_retry()`：创建新日志条目（状态：`Started`），更新 `current_log_id`
   - 清空 `on_token` 回调中的累积状态（通过创建新的回调闭包）

#### 成功时
1. 将当前日志标记为 `Completed`
2. 记录 token 使用量

### 4. 实现细节

#### 修改 translate_and_summarize 函数签名

```rust
pub async fn translate_and_summarize(
    config: &LlmProviderConfig,
    title: &str,
    content: &str,
    target_lang: &str,
    retry_ctx: &mut RetryContext,
) -> Result<(StreamResult, ParsedArticle), crate::error::AppError>
```

#### 重试循环

```rust
loop {
    let log_id = retry_ctx.current_log_id.clone().unwrap();
    let ot = mtok(retry_ctx.monitor.clone(), retry_ctx.feed_name.clone(), log_id);
    
    match chat_stream(config, SYSTEM_PROMPT, &full, ot).await {
        Ok(result) => {
            match parse_llm_output(&result.text) {
                Ok(parsed) => {
                    retry_ctx.mark_success(&result.usage);
                    return Ok((result, parsed));
                }
                Err(e) => {
                    retry_ctx.record_failure(e);
                    if !retry_ctx.should_retry() {
                        return Err(retry_ctx.last_error.take().unwrap());
                    }
                    retry_ctx.wait().await;
                    retry_ctx.prepare_retry();
                }
            }
        }
        Err(e) => {
            retry_ctx.record_failure(e);
            if !retry_ctx.should_retry() {
                return Err(retry_ctx.last_error.take().unwrap());
            }
            retry_ctx.wait().await;
            retry_ctx.prepare_retry();
        }
    }
}
```

#### 调用方修改（scheduler.rs）

```rust
let mut retry_ctx = RetryContext::new(
    config.llm.max_retries,
    config.llm.retry_delay_secs,
    feed_name.to_string(),
    raw.title.clone(),
    model.clone(),
    monitor.clone(),
);
retry_ctx.prepare_retry(); // 创建第一个日志条目

let result = translate_and_summarize(tc, &raw.title, &raw.content, target, &mut retry_ctx).await;
```

## 涉及文件

1. `src/config.rs` — 添加重试配置字段
2. `src/llm/mod.rs` — 添加 `RetryContext` 结构体
3. `src/llm/translate_summarize.rs` — 修改函数签名和重试逻辑
4. `src/scheduler.rs` — 修改调用方式

## 向后兼容性

- 新配置字段使用 `#[serde(default)]`，现有 config.toml 无需修改即可正常工作
- 默认重试次数保持 2 次（与当前硬编码值一致）
- 新增重试延迟默认 1 秒（当前无延迟）

## 测试策略

1. 单元测试：测试 `RetryContext` 的状态管理逻辑
2. 集成测试：测试重试流程的端到端行为
3. 手动测试：验证配置生效和日志记录正确性
