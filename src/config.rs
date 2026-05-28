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
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FeedConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port() -> u16 {
    3000
}
fn default_target_lang() -> String {
    "zho".into()
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

impl Config {
    pub fn load() -> Result<Self, crate::error::AppError> {
        let path = config_path();
        if !path.exists() {
            return Err(crate::error::AppError::Config(format!(
                "Config file not found at {}. Create one with your settings.",
                path.display()
            )));
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
# ISO 639-3 code for the target language
# zho = Chinese, eng = English, jpn = Japanese, etc.
target = "zho"

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

[[feeds]]
name = "rust-blog"
url = "https://blog.rust-lang.org/feed.xml"
enabled = true
interval_secs = 600

[logging]
level = "info"
"#
    .to_string()
}
