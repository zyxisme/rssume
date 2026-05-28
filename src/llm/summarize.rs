use super::{StreamResult, chat_stream};
use crate::config::LlmProviderConfig;

const SYSTEM_PROMPT: &str = r#"You are a skilled content summarizer. Write a concise one-sentence summary of the following article in Chinese.
Capture the key point. Output only the summary, nothing else."#;

pub async fn summarize(
    config: &LlmProviderConfig,
    title: &str,
    content: &str,
    on_token: impl FnMut(&str),
) -> Result<StreamResult, crate::error::AppError> {
    let text = if content.len() > 4000 {
        &content[..content.floor_char_boundary(4000)]
    } else {
        content
    };
    let prompt = format!("Title: {}\n\nContent:\n{}", title, text);
    let append = config.prompt_append.clone().unwrap_or_default();
    let full = if append.is_empty() {
        prompt
    } else {
        format!("{}\n{}", prompt, append)
    };
    chat_stream(config, SYSTEM_PROMPT, &full, on_token).await
}
