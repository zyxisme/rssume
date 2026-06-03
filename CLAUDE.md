# CLAUDE.md — rssume

RSS middleware with AI-powered translation and summarization, built in Rust.

## Project Overview

rssume monitors multiple RSS sources, detects article language, translates non-target-language articles via LLM API, prepends AI-generated summaries, and re-exports the processed feeds as RSS endpoints. A built-in web dashboard provides feed management, article browsing, statistics, and service control.

## Tech Stack

- **Runtime**: Tokio async
- **HTTP server**: Axum 0.8
- **HTTP client**: reqwest 0.12 (rustls)
- **RSS parsing**: feed-rs 2
- **RSS generation**: custom (axum XML response)
- **Templating**: Tera 1 (server-side rendering)
- **Language detection**: whatlang 0.16
- **Config/Data**: TOML (toml 0.8 + serde)
- **Frontend**: SSR HTML + htmx (no JS build toolchain)

## Architecture

```
src/
├── main.rs          # Entry point, server bootstrap, signal handling
├── config.rs        # TOML config parsing, env var expansion
├── error.rs         # Unified error type (thiserror)
├── lang.rs          # Language detection via whatlang
├── storage.rs       # TOML file read/write for article data
├── scheduler.rs     # Periodic RSS polling with configurable intervals
├── monitor.rs       # Shared runtime state (feed status, translation logs, token usage)
├── rss/
│   ├── mod.rs
│   ├── fetch.rs     # Fetch and parse RSS/Atom feeds
│   └── generate.rs  # Generate RSS 2.0 XML from stored articles
├── llm/
│   ├── mod.rs
│   ├── retry.rs     # RetryContext for configurable retry state management
│   └── translate_summarize.rs # LLM translation + summarization (OpenAI-compatible API)
├── opml.rs          # OPML export for feed subscriptions
└── web/
    ├── mod.rs       # Router composition
    ├── panel.rs     # Web dashboard pages (tera templates)
    ├── api.rs       # REST API for panel interactions
    └── rss_route.rs # RSS export endpoints (/feeds/:name)
```

## Config & Data Locations

| Platform | Config | Data |
|----------|--------|------|
| Linux | `~/.config/rssume/config.toml` | `~/.local/share/rssume/data/` |
| macOS | `~/Library/Application Support/rssume/config.toml` | `~/Library/Application Support/rssume/data/` |
| Windows | `%APPDATA%\rssume\config.toml` | `%APPDATA%\rssume\data\` |

Uses the `directories` crate for cross-platform paths.

## Web Panel Design

Design system: Vercel-inspired (see DESIGN.md)
- Light canvas with dark ink text
- Geist/Inter font family
- CSS variables for all tokens
- Server-side rendered with Tera + htmx for interactivity
- No JavaScript build step
- **htmx polling**: decouple trigger from content — empty div with `hx-get`/`hx-trigger` targets a
  separate `div id="..."` via `hx-target`. Never put polling attributes on the swapped content.
- **htmx auto-scroll**: use `hx-on:after-swap` on the trigger div. For streaming text, add
  scroll-stickiness guard: `if (el.scrollHeight - el.scrollTop - el.clientHeight < 50)` before
  setting `scrollTop = scrollHeight`, so users can scroll up without being yanked back.
- **htmx streaming tail**: for continuously growing text (e.g., LLM token stream), send full text
  from the API. Client-side: `white-space:pre; overflow-x:hidden; overflow-y:auto; max-height:200px`
  clips long lines at container width and scrolls vertically. The `hx-on:after-swap` auto-scroll
  handler keeps the view pinned to the latest content.
- **htmx streaming fixed-height**: use `height:Npx` (not `max-height`) for fixed-height streaming
  text. `overflow-y:auto` preserves scrolling for auto-scroll; `overflow-y:hidden` for no-scroll.
- **Tera last-N-lines**: truncate streamed text to last N lines in template:
  `{% set lines = text | split(pat="\n") %}{% set total = lines | length %}{% set start = total - N %}{% if start < 0 %}{% set start = 0 %}{% endif %}{{ lines | slice(start=start, end=total) | join(sep="\n") }}`
- **Monitor terminology**: "翻译状态窗口" = `.stream-text` (translation content), NOT `#active-translations` (container)

## LLM Integration

- OpenAI-compatible API protocol (supports OpenAI, DeepSeek, Groq, etc.)
- Translation and summarization can use different models/providers
- API keys resolved from environment variables in config (e.g., `${OPENAI_API_KEY}`)
- Exponential backoff on rate limits
- **Retry mechanism**: configurable via `[llm]` section — `max_retries` (default 2), `retry_delay_secs` (default 1). Each retry creates a new log entry; failed logs preserved.

## Build & Run

```bash
cargo fmt --check && cargo clippy --all-targets
cargo test
# Just run — config auto-created on first run, templates embedded at compile time
cargo build --release
./target/release/rssume
# Web panel at http://localhost:3000/panel
# RSS feeds at http://localhost:3000/feeds/:name
```

## CI

- Push/PR CI (`.github/workflows/ci.yml`): fmt → clippy → test(3os) + build(5 targets), Windows uses debug profile for speed
- Release CI (`.github/workflows/release.yml`): triggered by `v*` tags, builds 5 targets, publishes via `softprops/action-gh-release`
- Linux targets use `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` for fully static binaries
- aarch64-musl toolchain from `cross-tools/musl-cross` — binary names use full triple: `aarch64-unknown-linux-musl-gcc`
- `RUSTFLAGS: -Dwarnings` in CI — all warnings are fatal
- **Do not run local builds** — commit → push → check CI via API: `curl -s "https://api.github.com/repos/zyxisme/rssume/commits/<sha>/check-runs"`
- CI runners are faster and more reliable than local builds

## Deployment Model

- Single binary, zero external dependencies at runtime
- Templates embedded via `include_str!()` + `tera.add_raw_template()` at compile time
- Config auto-created on first run if missing (`~/.config/rssume/config.toml`)
- **Static assets**: embed via `include_str!()` and serve through dedicated endpoints (e.g., `/feeds/style.xsl` for XSLT)
- **XSLT pattern**: use `<?xml-stylesheet?>` processing instruction in XML endpoints for browser-friendly rendering
- **RSS Content-Type**: use `text/xml` (not `application/rss+xml`) — Chrome does not apply XSLT to `application/rss+xml`

## RSS Preview Page (XSL)

- **Content rendering**: `rss_style.xsl` uses `<xsl:value-of select="content:encoded" disable-output-escaping="yes"/>` to render HTML
- **Code block enhancements**: client-side JS wraps `<pre><code>` after `hljs.highlightElement()` — adds line numbers, language badge, copy button
- **XSL DOM manipulation**: use `DOMContentLoaded` listener; clone nodes with `cloneNode(true)` before replacing originals
- **Code block CSS**: `.code-body` uses `display:flex; overflow-x:auto`, `.line-numbers` has `flex-shrink:0; white-space:pre`
- **Clipboard pattern**: `navigator.clipboard.writeText()` → toggle button text/class → `setTimeout` reset after 2s
- **XSL inline JS escaping**: escape `<` as `&lt;` in JavaScript inside XSL templates (e.g., `i &lt;= len` not `i <= len`). Unescaped `<` breaks XML well-formedness and causes browsers to silently fail XSLT, rendering a blank page.
- **XSL validation**: when debugging blank RSS preview, validate XSL as XML first: `python3 -c "import xml.etree.ElementTree as ET; ET.fromstring(open('templates/rss_style.xsl').read())"`

## Code Conventions

- No unnecessary comments — code should be self-documenting
- Use `anyhow` for application errors, `thiserror` for library errors
- Module visibility: keep internals private, expose only what's needed
- Async everywhere: all I/O is async via tokio
- Single binary: no separate server/worker processes
- **Shared state**: `Arc<RwLock<Monitor>>` in `AppState { config, monitor }`, passed to scheduler + web
- **axum 0.8 state**: uses `Extension<Arc<AppState>>` (not `State`) for routers needing shared access
- **Static asset routes**: define static routes (e.g., `/feeds/style.xsl`) before parameterized routes (e.g., `/feeds/{name}`) to avoid conflicts
- **LLM streaming**: `resp.bytes_stream()` with SSE parsing, 60s idle timeout per chunk
- **Article compat**: new fields use `#[serde(default)]` so existing TOML data deserializes fine
- **Templates**: `include_str!()` at compile time + `tera.add_raw_template()` in `tera_instance()`
- **Target locale**: `zh_CN` POSIX format (not ISO 639-3 `zho`), passed directly to LLM in translate prompt
- **Lang detection**: `normalize_code()` splits on `-` and `_`, maps 11 codes (zh/en/ja/ko/fr/de/es/ru/ar/pt/it) to ISO 639-3
- **Self-referential URLs**: when generating URLs that point back to rssume (e.g. OPML `xmlUrl`),
  extract base URL from request headers: `X-Forwarded-Proto` + `X-Forwarded-Host` → `Host` header
  → config fallback. Never hardcode upstream URLs where rssume's own URL is needed.

## MCP Search Strategy

- **搜索/研究任务委托子代理**：需要多轮搜索或研究时，用 Agent 工具派遣子代理执行，子代理只返回 200-300 字摘要，原始内容不进入主会话上下文
- **简单搜索可直接调用**：单次、目标明确的搜索直接用 `tavily_search`，限制 `max_results: 3`、`search_depth: "fast"`
- **避免直接用 `tavily_research`**：该工具自动多轮搜索+全文汇总，上下文消耗极高
- **extract/crawl 限制参数**：`extract_depth: "basic"`、`limit: 10`、`max_depth: 1`

## Release & Versioning

- Default version bump: `0.0.1` unless user specifies otherwise
- Pre-publish: version bump → commit Cargo.toml + Cargo.lock → push to CI → wait for CI pass → `cargo publish` → push tag. Publish on a clean tree, don't use `--allow-dirty`.
- Release CI triggers on `v*` tag push, builds 5 targets, uploads to GitHub Releases
