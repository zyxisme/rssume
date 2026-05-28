use crate::config::LlmProviderConfig;

const SYSTEM_PROMPT: &str = r#"You are a professional translator. Translate the following text accurately into the target language.
Preserve all formatting, HTML tags, code blocks, and technical terms.
Only output the translated text, nothing else."#;

pub async fn translate(
    config: &LlmProviderConfig,
    text: &str,
    target_lang: &str,
) -> Result<String, crate::error::AppError> {
    let lang_name = crate::lang::lang_name(target_lang);
    let user_prompt = format!("Translate the following text to {}:\n\n{}", lang_name, text);

    let append = config.prompt_append.clone().unwrap_or_default();
    let full_prompt = if append.is_empty() {
        user_prompt
    } else {
        format!("{}\n{}", user_prompt, append)
    };

    super::chat(config, SYSTEM_PROMPT, &full_prompt).await
}
