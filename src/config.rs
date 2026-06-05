use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub language: LanguageConfig,
    pub llm: LlmConfig,
    pub feeds: Vec<FeedConfig>,
    #[serde(default)]
    pub logging: LogConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LanguageConfig {
    #[serde(default = "default_target_lang")]
    pub target: String,
}

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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LlmProviderConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    #[serde(default = "default_translation_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub prompt_append: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FeedConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    #[serde(default = "default_max_articles")]
    pub max_articles: usize,
    #[serde(default)]
    pub target_lang: Option<String>,
    #[serde(default)]
    pub prompt_append: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port() -> u16 {
    3000
}
fn default_target_lang() -> String {
    "zh_CN".into()
}
fn default_translation_base_url() -> String {
    "https://api.openai.com/v1".into()
}
fn default_enabled() -> bool {
    true
}
fn default_interval() -> u64 {
    300
}
fn default_log_level() -> String {
    "info".into()
}
fn default_max_concurrent_requests() -> usize {
    3
}
fn default_max_articles() -> usize {
    25
}
fn default_max_retries() -> u32 {
    2
}
fn default_retry_delay_secs() -> u64 {
    1
}

impl Config {
    pub fn load() -> Result<Self, crate::error::AppError> {
        let path = config_path();
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, default_config_toml())?;
            tracing::warn!("Created default config at {}", path.display());
            tracing::warn!("Edit this file to configure LLM API keys and feeds.");
        }
        let content = std::fs::read_to_string(&path)?;
        let resolved = resolve_env_vars(&content);
        let config: Config = toml::from_str(&resolved)?;
        Ok(config)
    }

    pub fn data_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "rssume")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("./data"))
    }
}

pub fn config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "rssume")
        .map(|d| d.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

fn resolve_env_vars(input: &str) -> String {
    let re = regex_lite::Regex::new(r"\$\{(\w+)\}").unwrap();
    re.replace_all(input, |caps: &regex_lite::Captures| {
        let var = &caps[1];
        std::env::var(var).unwrap_or_else(|_| format!("${{{}}}", var))
    })
    .to_string()
}

/// Generate a default config.toml content
pub fn default_config_toml() -> String {
    r#"# rssume configuration
[server]
host = "127.0.0.1"
port = 3000

[language]
# Target language — ISO code (zh_CN, en, ja) or free-form name (简体中文, English, 日本語)
target = "zh_CN"

[llm]
max_concurrent_requests = 3
max_retries = 2
retry_delay_secs = 1

[llm.translation]
provider = "openai"
model = "gpt-4o-mini"
# Use ${ENV_VAR} to reference environment variables
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
max_articles = 25
# target_lang = "简体中文"     # per-feed target language override (optional)
# prompt_append = "..."        # extra instructions injected into the LLM prompt (optional)

[[feeds]]
name = "rust-blog"
url = "https://blog.rust-lang.org/feed.xml"
enabled = true
interval_secs = 600
max_articles = 25
# target_lang = "English"
# prompt_append = "Focus on technical details."

[logging]
level = "info"
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_with_defaults() {
        let toml = r#"
[server]
host = "0.0.0.0"
port = 8080

[language]
target = "en"

[llm]
[llm.translation]
provider = "openai"
model = "gpt-4o"
api_key = "sk-test"

[llm.summary]
provider = "openai"
model = "gpt-4o"
api_key = "sk-test"

[[feeds]]
name = "test-feed"
url = "https://example.com/feed.xml"
"#;
        let config: Config = toml::from_str(toml).unwrap();

        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.language.target, "en");

        // LLM defaults
        assert_eq!(config.llm.max_concurrent_requests, 3);
        assert_eq!(config.llm.max_retries, 2);
        assert_eq!(config.llm.retry_delay_secs, 1);

        // LLM provider defaults
        assert_eq!(config.llm.translation.base_url, "https://api.openai.com/v1");
        assert!(config.llm.translation.prompt_append.is_none());
        assert!(config.llm.translation.max_tokens.is_none());
        assert_eq!(config.llm.summary.base_url, "https://api.openai.com/v1");
        assert!(config.llm.summary.prompt_append.is_none());
        assert!(config.llm.summary.max_tokens.is_none());

        // Feed defaults
        assert_eq!(config.feeds.len(), 1);
        assert!(config.feeds[0].enabled);
        assert_eq!(config.feeds[0].interval_secs, 300);
        assert_eq!(config.feeds[0].max_articles, 25);
        assert!(config.feeds[0].target_lang.is_none());
        assert!(config.feeds[0].prompt_append.is_none());

        // Logging defaults (entire section missing)
        assert_eq!(config.logging.level, "info");
    }

    #[test]
    fn default_config_toml_round_trip() {
        let toml_str = default_config_toml();
        let config: Config = toml::from_str(&toml_str).unwrap();

        // Verify the round-tripped config has expected values
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.language.target, "zh_CN");
        assert_eq!(config.llm.max_concurrent_requests, 3);
        assert_eq!(config.llm.max_retries, 2);
        assert_eq!(config.llm.retry_delay_secs, 1);
        assert_eq!(config.llm.translation.provider, "openai");
        assert_eq!(config.llm.translation.model, "gpt-4o-mini");
        assert_eq!(config.llm.translation.base_url, "https://api.openai.com/v1");
        assert_eq!(config.llm.summary.provider, "openai");
        assert_eq!(config.llm.summary.model, "gpt-4o-mini");
        assert_eq!(config.feeds.len(), 2);
        assert_eq!(config.feeds[0].name, "hacker-news");
        assert_eq!(config.feeds[1].name, "rust-blog");
        assert_eq!(config.logging.level, "info");

        // Verify re-serialization produces a valid TOML that parses back identically
        let reserialized = toml::to_string(&config).unwrap();
        let config2: Config = toml::from_str(&reserialized).unwrap();
        assert_eq!(config.server.host, config2.server.host);
        assert_eq!(config.server.port, config2.server.port);
        assert_eq!(config.language.target, config2.language.target);
        assert_eq!(
            config.llm.max_concurrent_requests,
            config2.llm.max_concurrent_requests
        );
        assert_eq!(config.feeds.len(), config2.feeds.len());
        assert_eq!(config.logging.level, config2.logging.level);
    }
}
