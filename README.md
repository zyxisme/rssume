# rssume

RSS middleware with AI-powered translation and summarization.

Monitor multiple RSS feeds → auto-detect language → translate via LLM → add AI summaries → export enhanced RSS feeds. Built-in web dashboard with Vercel-inspired design.

## Features

- Multi-source RSS monitoring with configurable polling intervals
- Automatic language detection (whatlang)
- LLM-powered translation for non-target-language articles
- AI-generated one-sentence summaries prepended to each article
- One-to-one RSS export via HTTP endpoints
- Web dashboard: feed management, article browsing, statistics, service control
- Multi-provider LLM support (OpenAI-compatible API)
- Cross-platform (Linux, macOS, Windows)
- Single binary, zero runtime dependencies

## Quick Start

```bash
# Install
cargo install rssume

# Create config
mkdir -p ~/.config/rssume
rssume --print-default-config > ~/.config/rssume/config.toml
# Edit config.toml: set API keys, add RSS feeds

# Run
rssume
# Web panel at http://localhost:3000/panel
# RSS feeds at http://localhost:3000/feeds/:name
```

## Configuration

Edit `~/.config/rssume/config.toml`:

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
git clone https://github.com/rssume/rssume
cd rssume
cargo build --release
```

## License

MIT
