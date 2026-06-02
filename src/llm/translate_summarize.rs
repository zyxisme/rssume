use super::{StreamResult, chat_stream};
use crate::config::LlmProviderConfig;

const MAX_RETRIES: u32 = 2;

const SYSTEM_PROMPT: &str = r#"Translate the article to the target language. Then write a one-sentence summary — just what the article is about, no filler.

Output format (each tag on its own line):

|||TITLE|||
<translated title>
|||END_TITLE|||

|||CONTENT|||
<translated content>
|||END_CONTENT|||

|||SUMMARY|||
<summary, one sentence, under 30 words>
|||END_SUMMARY|||

Keep HTML tags and code blocks intact."#;

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
        "Target language: {}\n\nTitle: {}\n\nContent:\n{}",
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
            Ok(parsed) => return Ok((result, parsed)),
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
