# 并发翻译支持设计

## 目标

让 rssume 支持并发处理多个 Feed 和多篇文章，提升翻译吞吐量，同时通过可配置的并发限制防止 LLM API 限流。

## 当前状态

- `process_all()` 串行处理每个 Feed
- `process_feed()` 串行处理每篇文章
- 每篇文章内部：标题翻译 → 内容翻译 → 摘要生成，全部串行
- Monitor 使用 `Arc<RwLock<Monitor>>`，已支持并发访问

## 设计

### 1. 配置层

在 `config.toml` 中添加全局并发限制：

```toml
[llm]
max_concurrent_requests = 3  # 默认 3，最大并发 LLM 请求数
```

**文件改动**：
- `src/config.rs`: 在 `LlmConfig` 中添加 `max_concurrent_requests` 字段

### 2. Scheduler 层

#### 2.1 并发架构

```
Scheduler::run_loop()
└── process_all()
    └── join_all([feed1, feed2, ...])  ← Feed 并发
        └── process_feed()
            └── process_articles_concurrent()
                └── join_all([article1, article2, ...])  ← 文章并发
                    └── process_single_article()
                        ├── translate_title()   ← 获取信号量
                        ├── translate_content() ← 获取信号量
                        └── summarize()         ← 获取信号量
```

#### 2.2 信号量管理

- 在 `Scheduler` 中持有 `Arc<Semaphore>`
- 信号量在所有 Feed 间共享
- 每次 LLM 调用前 `acquire()`，完成后自动释放
- 初始化：`Semaphore::new(config.llm.max_concurrent_requests)`

#### 2.3 文章处理流程

每篇文章的内部步骤保持串行（有依赖关系）：
1. 翻译标题（如果需要）
2. 翻译内容（如果需要，使用翻译后的标题）
3. 生成摘要（使用翻译后的标题和内容）

**文件改动**：
- `src/scheduler.rs`: 重构为并发处理

### 3. Monitor 层

#### 3.1 状态追踪

当前 `FeedStatus::Translating` 语义在并发下不清晰：

```rust
// 当前
Translating {
    current: u32,         // 第几篇（串行才有意义）
    total: u32,
    current_title: String,
}

// 改为
Translating {
    completed: u32,           // 已完成文章数
    total: u32,               // 总文章数
    in_progress: Vec<String>, // 正在处理的文章标题
}
```

#### 3.2 并发写入安全

- `Monitor` 已使用 `Arc<RwLock<Monitor>>`，支持并发写入
- 多个文章任务可能同时调用 `update_log()`、`add_token_usage()` 等
- RwLock 保证数据一致性

**文件改动**：
- `src/monitor.rs`: 修改 `FeedStatus::Translating` 结构

### 4. 模板层

#### 4.1 Monitor 页面适配

显示并发翻译进度：

```html
{% if feed.status is starting_with('Translating') %}
<span>Translating {{ feed.completed }}/{{ feed.total }} ({{ feed.in_progress | length }} in progress)</span>
{% for title in feed.in_progress %}
<span class="badge">{{ title | truncate(length=30) }}</span>
{% endfor %}
{% endif %}
```

#### 4.2 全局 Monitor 页面

同样需要适配新的状态结构。

**文件改动**：
- `templates/partials/feed_monitor_status.html`
- `templates/partials/monitor_status.html`
- `templates/feed_monitor.html`（如果需要）
- `templates/monitor.html`（如果需要）

### 5. 错误处理

- 单篇文章失败不影响其他文章
- 使用 `join_all` 收集所有结果，记录失败的文章
- Feed 级别：即使部分文章失败，Feed 状态仍为 `Done`

### 6. 限流保护

- 信号量限制总并发数（已实现）
- 指数退避：复用现有 `translate.rs` 和 `summarize.rs` 中的错误重试逻辑
- 不额外添加请求间隔，依赖信号量控制并发

## 不改动

- LLM 调用逻辑（`translate.rs`、`summarize.rs`）
- RSS 抓取逻辑（`fetch.rs`）
- 存储逻辑（`storage.rs`）
- Web API 接口（`api.rs`）

## 验证

1. 配置 `max_concurrent_requests = 5`
2. 添加多个 Feed，每个有 5+ 篇新文章
3. 观察 Monitor 页面：
   - 多个 Feed 同时显示 "Translating"
   - 每个 Feed 内显示多篇 "in progress"
4. 检查日志：并发 LLM 请求数不超过配置值
5. 验证最终结果：所有文章正确翻译和存储

## 风险

1. **LLM API 限流**：通过信号量控制，可配置
2. **Monitor 写入竞争**：RwLock 已处理，性能可接受
3. **内存使用**：并发处理会增加内存，但文章数量有限
4. **复杂度增加**：需要更仔细的错误处理和状态管理
