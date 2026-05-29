use super::{StreamResult, chat_stream};
use crate::config::LlmProviderConfig;

const MAX_RETRIES: u32 = 2;

const SYSTEM_PROMPT: &str = r#"You are a professional translator and summarizer. Translate the article below into the target language, then generate a concise summary.

Output in this EXACT format (tags must be on their own line):

|||TITLE|||
<translated title>
|||END_TITLE|||

|||CONTENT|||
<translated content>
|||END_CONTENT|||

|||SUMMARY|||
<concise summary in target language, approximately 150 words>
|||END_SUMMARY|||

Important:
- Tags like |||TITLE||| must be on their own line with no other content
- Preserve all formatting, HTML tags, code blocks, and technical terms
- Do not add any text outside the tagged sections"#;

#[derive(Debug, Default)]
pub struct ParsedArticle {
    pub title: Option<String>,
    pub content: Option<String>,
    pub summary: Option<String>,
}

enum Section {
    Title,
    Content,
    Summary,
}

pub async fn translate_and_summarize(
    config: &LlmProviderConfig,
    title: &str,
    content: &str,
    target_lang: &str,
    mut on_token: impl FnMut(&str),
) -> Result<(StreamResult, ParsedArticle), crate::error::AppError> {
    let prompt = format!(
        "Translate the following article to {}:\n\nTitle: {}\n\nContent:\n{}",
        target_lang, title, content
    );
    let append = config.prompt_append.clone().unwrap_or_default();
    let full = if append.is_empty() {
        prompt
    } else {
        format!("{}\n{}", prompt, append)
    };

    let mut last_error = None;

    for attempt in 0..=MAX_RETRIES {
        let result = match chat_stream(config, SYSTEM_PROMPT, &full, &mut on_token).await {
            Ok(r) => r,
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        };

        match parse_llm_output(&result.text) {
            Ok(parsed) => {
                // Check if translated content is in target language
                if let Some(ref content_text) = parsed.content
                    && crate::lang::needs_translation(content_text, target_lang)
                {
                    tracing::warn!(
                        "Attempt {}/{}: Translated content not in target language, retrying",
                        attempt + 1,
                        MAX_RETRIES + 1
                    );
                    last_error = Some(crate::error::AppError::Llm(
                        "Translated content not in target language".into(),
                    ));
                    continue;
                }
                return Ok((result, parsed));
            }
            Err(e) => {
                tracing::warn!(
                    "Attempt {}/{}: Failed to parse LLM output: {}, retrying",
                    attempt + 1,
                    MAX_RETRIES + 1,
                    e
                );
                last_error = Some(e);
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| crate::error::AppError::Llm("All retry attempts failed".into())))
}

fn parse_llm_output(text: &str) -> Result<ParsedArticle, crate::error::AppError> {
    let mut result = ParsedArticle::default();
    let mut current_section: Option<Section> = None;
    let mut buffer = Vec::new();

    for line in text.lines() {
        match line.trim() {
            "|||TITLE|||" => {
                flush_buffer(&mut buffer, &mut current_section, &mut result);
                current_section = Some(Section::Title);
            }
            "|||END_TITLE|||" => {
                flush_buffer(&mut buffer, &mut current_section, &mut result);
                current_section = None;
            }
            "|||CONTENT|||" => {
                flush_buffer(&mut buffer, &mut current_section, &mut result);
                current_section = Some(Section::Content);
            }
            "|||END_CONTENT|||" => {
                flush_buffer(&mut buffer, &mut current_section, &mut result);
                current_section = None;
            }
            "|||SUMMARY|||" => {
                flush_buffer(&mut buffer, &mut current_section, &mut result);
                current_section = Some(Section::Summary);
            }
            "|||END_SUMMARY|||" => {
                flush_buffer(&mut buffer, &mut current_section, &mut result);
                current_section = None;
            }
            _ => {
                if current_section.is_some() {
                    buffer.push(line.to_string());
                }
            }
        }
    }
    flush_buffer(&mut buffer, &mut current_section, &mut result);

    if result.title.is_none() && result.content.is_none() {
        return Err(crate::error::AppError::Llm(
            "Failed to parse LLM output: no title or content found".into(),
        ));
    }

    Ok(result)
}

fn flush_buffer(
    buffer: &mut Vec<String>,
    section: &mut Option<Section>,
    result: &mut ParsedArticle,
) {
    if buffer.is_empty() {
        return;
    }
    let text = buffer.join("\n").trim().to_string();
    match section {
        Some(Section::Title) => result.title = Some(text),
        Some(Section::Content) => result.content = Some(text),
        Some(Section::Summary) => result.summary = Some(text),
        None => {}
    }
    buffer.clear();
}
