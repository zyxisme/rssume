# rssume 监控面板 + 流式 LLM + RSS 兼容性增强 — 设计文档

## 1. 问题根因

| 问题 | 根因 | 位置 |
|------|------|------|
| 面板数据不更新 | `dashboard()` 调用 `Config::load()` 直接读磁盘，与 scheduler 持有的 `Arc<RwLock<Config>>` 不同步；面板无自动刷新 | `web/panel.rs:32` |
| 标题不翻译 | `scheduler.rs:85` 始终 `title: raw.title.clone()`，从未调 LLM 翻译标题 | `scheduler.rs:85` |
| LLM 长文章超时 | `chat()` 非流式 API，120s 固定超时 | `llm/mod.rs:41-42` |
| 面板状态不完善 | `FeedStats` 只有 3 个计数（文章/翻译/摘要），无拉取状态、时间、错误 | `storage.rs:96-102` |
| RSS 兼容性差 | 只输出基础 RSS 2.0 字段，缺 `content:encoded`、`dc:creator`、`category`、`enclosure` | `rss/generate.rs` |

## 2. 架构变更

新增 `monitor` 模块作为 scheduler 和 web 间的共享状态桥梁：

```
src/
├── monitor.rs          # NEW — Arc<RwLock<Monitor>> 内存状态管理
├── main.rs             # 创建 Monitor，同时传给 scheduler 和 web router
├── scheduler.rs        # 每次拉取/翻译时更新 Monitor
├── llm/
│   ├── mod.rs          # chat() → chat_stream() 流式 + 回调
│   ├── translate.rs    # 翻译时推送 token 到 Monitor
│   └── summarize.rs    # 摘要同理
├── web/
│   ├── mod.rs
│   ├── panel.rs        # 新增 /panel/monitor + /panel/feed/{name}/logs
│   ├── api.rs          # 新增 monitor 相关 API
│   └── rss_route.rs
├── rss/
│   ├── mod.rs
│   ├── fetch.rs        # RawArticle 扩展 author/categories/media
│   └── generate.rs     # content:encoded + dc:creator + category + enclosure
└── storage.rs          # FeedStats 增加状态字段；Article 增加 author/categories
```

**数据流：**
```
scheduler → monitor.feeds[name].status = Fetching/Translating
scheduler → llm::chat_stream(on_token) → monitor.translation_logs[id].Streaming
web API GET /api/monitor/translating → htmx 2s 轮询实时展示
```

## 3. Monitor 数据结构

```rust
pub struct Monitor {
    pub feeds: HashMap<String, FeedRuntimeState>,            // 纯内存
    pub translation_logs: HashMap<String, VecDeque<TranslationLog>>, // 纯内存，每 feed 最多 500 条
    pub token_usage: TokenUsage,                              // 持久化
}

pub struct FeedRuntimeState {
    pub status: FeedStatus,
    pub last_fetch_at: Option<String>,
    pub last_fetch_error: Option<String>,
    pub last_poll_duration_ms: u64,
}

pub enum FeedStatus {
    Idle,
    Fetching,
    Translating { current: u32, total: u32, current_title: String },
    Done,
    Error(String),
}

pub struct TranslationLog {
    pub timestamp: String,
    pub article_title: String,
    pub stage: TranslationStage,
    pub status: LogStatus,
    pub model: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

pub enum TranslationStage { TranslatingTitle, TranslatingContent, Summarizing }
pub enum LogStatus { Started, Streaming { tokens: String }, Completed, Failed(String) }

pub struct TokenUsage {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub by_model: HashMap<String, ModelUsage>,
    pub by_feed: HashMap<String, FeedTokenUsage>,
}
```

**持久化策略：**
- Monitor / feeds runtime state / translation_logs → 纯内存，重启清空
- TokenUsage → 持久化到 `data/token_usage.toml`，初始化加载，每次 LLM 调用完成后写入

## 4. LLM 流式改造

`chat_stream(config, system, user, on_token)`:
- POST `{ stream: true }`，逐行读取 SSE `data:` chunk
- 每个 `delta.content` → `on_token(content)` 回调 → monitor 实时累积
- 连接超时 30s，空闲超时 60s（60s 无 token 断连），移除总超时
- 返回 `StreamResult { text, usage: UsageInfo }`
- UsageInfo 来自最后一个 chunk 的 `usage` 字段：`{ prompt_tokens, completion_tokens, total_tokens }`

translate.rs / summarize.rs:
- 标题翻译：content 需要翻译时标题也翻译（新增一次 LLM 调用）
- 接收 monitor 引用，回调中写入 `translation_logs`
- 返回 token 用量供累加

## 5. 面板 UI

**路由扩展：**

| 路由 | 说明 | 刷新方式 |
|------|------|----------|
| `GET /panel` | Dashboard（改读共享 Arc config） | htmx 30s |
| `GET /panel/feed/{name}` | Feed 详情 | 手动 |
| `GET /panel/monitor` | **新增** 实时翻译监控 | htmx 2s |
| `GET /panel/feed/{name}/logs` | **新增** 历史翻译日志 | 手动 |
| `GET /api/monitor/status` | **新增** 运行时状态 JSON | htmx 15s |
| `GET /api/monitor/translating` | **新增** 当前翻译实时 token | htmx 2s |
| `GET /api/token-usage` | **新增** Token 用量 JSON | 手动 |

**Dashboard 改造：**
- 5 个 stat cards：Feeds / Articles / Translated / Summarized / Token 消耗
- Feed 表格每行增加状态指示灯（● 正常 / ◐ 拉取中 / ● 错误）+ 最后拉取时间 + 错误信息
- 配置从共享 `Arc<RwLock<Config>>` 读取，不再直接读磁盘
- 加 `hx-trigger="every 30s"` 自动刷新统计区

**导航栏：** Dashboard | Monitor | Settings

**/panel/monitor — 实时翻译视图：**
- 上区：当前正在翻译的文章（title, stage, model, 流式 token 输出, 实时 token 计数）
- 下区：最近完成列表（时间, 文章标题, 模型, 输入/输出 token 数）
- `hx-trigger="every 2s"` 拉取 /api/monitor/translating

**/panel/feed/{name}/logs — 历史日志：**
- 按时间倒序列出该 feed 所有翻译日志
- 每条：时间、文章标题、阶段、状态（成功/失败）、模型、token 消耗

## 6. RSS 兼容性增强

**fetch.rs — RawArticle 扩展：**
```rust
pub struct RawArticle {
    pub title: String,
    pub link: String,
    pub content: String,
    pub published_at: String,
    pub author: Option<String>,       // NEW: Person.name
    pub categories: Vec<String>,      // NEW: Category.term
    pub guid: Option<String>,         // NEW: entry.id (origin guid)
    pub media_urls: Vec<MediaItem>,   // NEW: from media content/thumbnails
}
```

**generate.rs — RSS 输出增强：**
- `<description>` 改为 AI 摘要（无摘要则截取前 200 字纯文本）
- `<content:encoded>` 放完整正文 HTML（含 AI 摘要前置 + 翻译标注），用 CDATA
- `<dc:creator>` 有 author 则输出
- `<category>` 每个 category 一个元素
- `<enclosure>` 有 media_urls 则输出第一张图（含 url/length/type）
- `<guid isPermaLink="false">` 用原始 guid 或 UUID
- `<atom:link rel="self">` 修复 href 为完整路径

**storage.rs — Article 扩展：**
```rust
pub struct Article {
    // ... 现有字段
    pub author: Option<String>,       // NEW
    pub categories: Vec<String>,      // NEW
    pub guid: String,                 // NEW: 替代 id（或保留 id，新增 guid）
}
```

**namespace 声明（rss 根元素）：**
```xml
<rss version="2.0"
     xmlns:atom="http://www.w3.org/2005/Atom"
     xmlns:content="http://purl.org/rss/1.0/modules/content/"
     xmlns:dc="http://purl.org/dc/elements/1.1/"
     xmlns:media="http://search.yahoo.com/mrss/">
```

## 7. 实现顺序

1. **monitor.rs** — 新建模块，定义所有数据结构 + `Arc<RwLock<Monitor>>` 构造
2. **llm/mod.rs** — `chat()` 改为 `chat_stream()`，SSE 解析 + on_token 回调
3. **llm/translate.rs + summarize.rs** — 适配流式接口 + 标题翻译 + token 回传
4. **scheduler.rs** — 集成 Monitor（更新 FeedStatus + TranslationLog + TokenUsage）
5. **rss/fetch.rs** — RawArticle 扩展 author/categories/guid/media
6. **rss/generate.rs** — content:encoded + dc:creator + category + enclosure
7. **storage.rs** — Article 扩展 + FeedStats 增加状态字段 + TokenUsage 持久化
8. **web/api.rs** — 新增 monitor 状态 API + token-usage API
9. **web/panel.rs** — 新增 monitor/logs 页面 + dashboard 改读共享 config
10. **templates** — dashboard/monitor/logs 模板
11. **main.rs** — 创建 Monitor，注入到 scheduler 和 web router
