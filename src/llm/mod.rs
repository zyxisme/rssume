pub mod retry;
pub mod translate_summarize;

use crate::config::LlmProviderConfig;
use futures_util::StreamExt;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
}

#[derive(Debug, serde::Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
    usage: Option<UsageInfo>,
}

#[derive(Debug, serde::Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub struct StreamResult {
    pub text: String,
    pub usage: UsageInfo,
}

#[allow(dead_code)]
pub async fn chat(
    config: &LlmProviderConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<StreamResult, crate::error::AppError> {
    chat_stream(config, system_prompt, user_prompt, |_| {}).await
}

pub async fn chat_stream(
    config: &LlmProviderConfig,
    system_prompt: &str,
    user_prompt: &str,
    mut on_token: impl FnMut(&str),
) -> Result<StreamResult, crate::error::AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| crate::error::AppError::Llm(format!("client: {}", e)))?;

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
        temperature: 0.1,
        max_tokens: config.max_tokens,
        stream: true,
    };

    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    tracing::info!(
        url = url.as_str(),
        model = config.model.as_str(),
        system_prompt_len = system_prompt.len(),
        user_prompt_len = user_prompt.len(),
        max_tokens = config.max_tokens,
        "LLM request"
    );
    tracing::debug!(
        system_prompt = system_prompt,
        user_prompt = user_prompt,
        "LLM request body"
    );

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| crate::error::AppError::Llm(format!("request: {}", e)))?;

    if !resp.status().is_success() {
        let s = resp.status();
        let t = resp.text().await.unwrap_or_default();
        tracing::error!(status = %s, body = t.as_str(), "LLM API error");
        return Err(crate::error::AppError::Llm(format!("API {}: {}", s, t)));
    }

    let mut stream = resp.bytes_stream();
    let mut full_text = String::new();
    let mut usage: Option<UsageInfo> = None;
    let mut finish_reason: Option<String> = None;
    let mut buf = String::new();

    loop {
        let chunk =
            match tokio::time::timeout(std::time::Duration::from_secs(60), stream.next()).await {
                Ok(Some(Ok(b))) => b,
                Ok(Some(Err(e))) => {
                    tracing::error!(error = %e, "LLM stream error");
                    return Err(crate::error::AppError::Llm(format!("stream: {}", e)));
                }
                Ok(None) => break,
                Err(_) => {
                    tracing::error!("LLM stream idle timeout");
                    return Err(crate::error::AppError::Llm("idle timeout".into()));
                }
            };

        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            buf = buf[nl + 1..].to_string();
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    continue;
                }
                if let Ok(c) = serde_json::from_str::<StreamChunk>(data) {
                    if let Some(choice) = c.choices.first() {
                        if let Some(ref ct) = choice.delta.content {
                            full_text.push_str(ct);
                            on_token(ct);
                        }
                        if let Some(ref fr) = choice.finish_reason {
                            finish_reason = Some(fr.clone());
                        }
                    }
                    if let Some(u) = c.usage {
                        usage = Some(u);
                    }
                }
            }
        }
    }

    if finish_reason.as_deref() == Some("length") {
        tracing::warn!(
            text_len = full_text.len(),
            max_tokens = config.max_tokens,
            "LLM response truncated: max_tokens limit reached — using partial result"
        );
    }

    let usage = usage.unwrap_or(UsageInfo {
        prompt_tokens: 0,
        completion_tokens: full_text.len() as u32 / 4,
        total_tokens: 0,
    });

    tracing::info!(
        text_len = full_text.len(),
        prompt_tokens = usage.prompt_tokens,
        completion_tokens = usage.completion_tokens,
        total_tokens = usage.total_tokens,
        finish_reason = finish_reason.as_deref().unwrap_or("none"),
        "LLM response"
    );
    tracing::debug!(text = full_text.as_str(), "LLM response text");

    Ok(StreamResult {
        text: full_text,
        usage,
    })
}
