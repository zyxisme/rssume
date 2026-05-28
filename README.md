<div align="center">

# rssume

[![License](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-%23dea584?style=for-the-badge)](https://www.rust-lang.org)
[![CI](https://img.shields.io/badge/CI-passing-brightgreen?style=for-the-badge)](https://github.com/zyxisme/rssume/actions/workflows/ci.yml)

**AI 驱动的 RSS 翻译与摘要中间件。**

订阅源 → 语言检测 → LLM 翻译 → AI 摘要 → 导出增强 RSS

</div>

## 功能

- **多源订阅** — 支持多个 RSS 源，可独立配置轮询间隔
- **语言检测** — 基于 whatlang 的自动语言识别，覆盖 70+ 语言
- **AI 翻译** — LLM 驱动的翻译，将文章转为目标语言（兼容 OpenAI API）
- **AI 摘要** — 每篇文章自动生成一句话 TL;DR，前置插入正文
- **RSS 导出** — 一对一的 HTTP 端点，兼容所有 RSS 阅读器
- **Web 控制台** — 订阅源管理、文章浏览、统计数据、服务控制
- **多 LLM 厂商** — 支持 OpenAI、DeepSeek、Groq 等兼容接口
- **单二进制** — 零运行时依赖，跨平台（Linux / macOS / Windows）

## 快速开始

```bash
# 安装
cargo install rssume

# 首次运行自动生成默认配置
rssume
```

浏览器打开 `http://localhost:3000/panel`，在控制台中配置订阅源和 LLM 即可。

RSS 订阅地址：`http://localhost:3000/feeds/:name`

## 配置

首次运行自动创建 `~/.config/rssume/config.toml`：

```toml
[server]
host = "127.0.0.1"
port = 3000

[language]
target = "zho"  # ISO 639-3: zho=中文, eng=英文

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

API Key 使用 `${ENV_VAR}` 语法引用环境变量，避免明文写入。

## 从源码构建

```bash
git clone https://github.com/zyxisme/rssume
cd rssume
cargo build --release
```

## 开源协议

MIT
