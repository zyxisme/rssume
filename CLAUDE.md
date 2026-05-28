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
├── rss/
│   ├── mod.rs
│   ├── fetch.rs     # Fetch and parse RSS/Atom feeds
│   └── generate.rs  # Generate RSS 2.0 XML from stored articles
├── llm/
│   ├── mod.rs
│   ├── translate.rs # LLM translation (OpenAI-compatible API)
│   └── summarize.rs # LLM summarization (OpenAI-compatible API)
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

## LLM Integration

- OpenAI-compatible API protocol (supports OpenAI, DeepSeek, Groq, etc.)
- Translation and summarization can use different models/providers
- API keys resolved from environment variables in config (e.g., `${OPENAI_API_KEY}`)
- Exponential backoff on rate limits

## Build & Run

```bash
cargo build --release
# Binary at target/release/rssume

# Create default config
mkdir -p ~/.config/rssume
cp config.toml ~/.config/rssume/

# Run
./target/release/rssume
# Web panel at http://localhost:3000/panel
# RSS feeds at http://localhost:3000/feeds/:name
```

## Cross-Platform CI

GitHub Actions builds for `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.

## Code Conventions

- No unnecessary comments — code should be self-documenting
- Use `anyhow` for application errors, `thiserror` for library errors
- Module visibility: keep internals private, expose only what's needed
- Async everywhere: all I/O is async via tokio
- Single binary: no separate server/worker processes
