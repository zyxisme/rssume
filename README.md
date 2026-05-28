<div align="center">

# rssume

[![License](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge&logo=open-source-initiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-%23dea584?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![CI](https://img.shields.io/badge/CI-passing-brightgreen?style=for-the-badge&logo=githubactions&logoColor=white)](https://github.com/zyxisme/rssume/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/rssume.svg?style=for-the-badge&logo=rust&logoColor=white)](https://crates.io/crates/rssume)

**AI 驱动的 RSS 翻译与摘要中间件。**

订阅源 → 语言检测 → LLM 流式翻译 → AI 摘要 → 增强 RSS

</div>

## 功能

- **多源订阅** — 支持多个 RSS 源，可独立配置轮询间隔

- **AI 翻译** — LLM 流式翻译，将文章标题和正文转为目标语言（兼容 OpenAI API）
- **AI 摘要** — 每篇文章自动生成一句话 TL;DR，前置插入正文
- **RSS 增强导出** — content:encoded、dc:creator、category、enclosure 等扩展字段，兼容所有主流 RSS 阅读器
- **Web 控制台** — 订阅源管理、文章浏览、统计数据、服务控制
- **实时翻译监控** — `/panel/monitor` 实时展示翻译进度、LLM token 流输出、历史日志
- **Token 用量统计** — 按模型和订阅源统计 token 消耗，持久化保存

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
target = "zh_CN"  # POSIX locale，直接传给 LLM

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

## 面板路由

| 路由 | 说明 |
|------|------|
| `/panel` | Dashboard — 订阅源概览、文章统计、token 消耗 |
| `/panel/monitor` | 实时翻译监控 — 当前翻译 token 流输出 |
| `/panel/feed/:name` | 文章详情 — 正文、翻译、原始内容 |
| `/panel/feed/:name/logs` | 翻译日志 — 历史记录、模型、token 用量 |
| `/panel/settings` | 配置概览 |

## 从源码构建

```bash
git clone https://github.com/zyxisme/rssume
cd rssume
cargo build --release
```

## 开源协议

MIT
