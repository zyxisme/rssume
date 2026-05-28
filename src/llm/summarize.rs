use crate::config::LlmProviderConfig;

const SYSTEM_PROMPT: &str = r#"You are a skilled content summarizer. Write a concise one-sentence summary of the following article in Chinese.
Capture the key point. Output only the summary, nothing else."#;

pub async fn summarize(
    config: &LlmProviderConfig,
    title: &str,
    content: &str,
) -> Result<String, crate::error::AppError> {
    let text = if content.len() > 4000 {
        &content[..4000]
    } else {
        content
    };

    let user_prompt = format!("Title: {}\n\nContent:\n{}", title, text);

    let append = config.prompt_append.clone().unwrap_or_default();
    let full_prompt = if append.is_empty() {
        user_prompt
    } else {
        format!("{}\n{}", user_prompt, append)
    };

    super::chat(config, SYSTEM_PROMPT, &full_prompt).await
}
