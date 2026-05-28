pub mod summarize;
pub mod translate;

use crate::config::LlmProviderConfig;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, serde::Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, serde::Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

#[derive(Debug, serde::Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

async fn chat(
    config: &LlmProviderConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, crate::error::AppError> {
    let client = reqwest::Client::new();
    let body = ChatRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_prompt.into(),
            },
        ],
        temperature: 0.3,
        max_tokens: 2048,
    };

    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| crate::error::AppError::Llm(format!("LLM request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(crate::error::AppError::Llm(format!(
            "LLM API error {}: {}",
            status, text
        )));
    }

    let data: ChatResponse = resp
        .json()
        .await
        .map_err(|e| crate::error::AppError::Llm(format!("Failed to parse LLM response: {}", e)))?;

    data.choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| crate::error::AppError::Llm("Empty response from LLM".into()))
}
