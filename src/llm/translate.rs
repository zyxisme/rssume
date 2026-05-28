use crate::config::LlmProviderConfig;
use super::{chat_stream, StreamResult};

const SYSTEM_PROMPT: &str = r#"You are a professional translator. Translate the following text accurately into the target language.
Preserve all formatting, HTML tags, code blocks, and technical terms.
Only output the translated text, nothing else."#;

pub async fn translate(
    config: &LlmProviderConfig, text: &str, target_lang: &str,
    on_token: impl FnMut(&str),
) -> Result<StreamResult, crate::error::AppError> {
    let prompt = format!("Translate the following text to {}:\n\n{}", target_lang, text);
    let append = config.prompt_append.clone().unwrap_or_default();
    let full = if append.is_empty() { prompt } else { format!("{}\n{}", prompt, append) };
    chat_stream(config, SYSTEM_PROMPT, &full, on_token).await
}
