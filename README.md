<div align="center">

# rssume

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-%23dea584?style=flat)](https://www.rust-lang.org)
[![CI](https://github.com/zyxisme/rssume/actions/workflows/ci.yml/badge.svg)](https://github.com/zyxisme/rssume/actions/workflows/ci.yml)

**RSS middleware with AI-powered translation and summarization.**

Monitor feeds → detect language → translate via LLM → add AI summaries → re-export as RSS

</div>

## Features

- **Multi-source RSS** — monitor multiple feeds with configurable polling intervals
- **Language detection** — automatic recognition via whatlang, 70+ languages
- **AI translation** — LLM-powered translation to your target language (OpenAI-compatible API)
- **AI summaries** — one-sentence TL;DR prepended to every article
- **RSS export** — one-to-one HTTP endpoints, compatible with any RSS reader
- **Web dashboard** — feed management, article browsing, statistics, service control
- **Multi-provider LLM** — OpenAI, DeepSeek, Groq, or any OpenAI-compatible endpoint
- **Single binary** — zero runtime dependencies, cross-platform (Linux / macOS / Windows)

## Quick Start

```bash
# Install from source
cargo install rssume

# Run — auto-creates default config on first launch
rssume
```

Open `http://localhost:3000/panel` in your browser, configure your feeds and LLM settings directly from the dashboard.

RSS feeds are exported at `http://localhost:3000/feeds/:name`.

## Configuration

`~/.config/rssume/config.toml` (auto-created on first run):

```toml
[server]
host = "127.0.0.1"
port = 3000

[language]
target = "zho"  # ISO 639-3: zho=Chinese, eng=English

[llm.translation]
provider = "openai"
model = "gpt-4o-mini"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"

[llm.summary]
provider = "openai"
model = "gpt-4o-mini"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"

[[feeds]]
name = "hacker-news"
url = "https://hnrss.org/frontpage"
enabled = true
interval_secs = 300
```

Use `${ENV_VAR}` syntax to reference environment variables for API keys.

## Build from Source

```bash
git clone https://github.com/zyxisme/rssume
cd rssume
cargo build --release
```

## License

MIT
