use super::{StreamResult, chat_stream};
use crate::config::LlmProviderConfig;
use crate::monitor::LogStatus;

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
    retry_ctx: &mut super::retry::RetryContext,
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

    loop {
        retry_ctx.prepare_retry().await;
        let log_id = retry_ctx.current_log_id().unwrap().to_string();
        let monitor = retry_ctx.monitor.clone();
        let feed_name = retry_ctx.feed_name.clone();

        let ot = move |t: &str| {
            let m = monitor.clone();
            let f = feed_name.clone();
            let l = log_id.clone();
            let s = t.to_string();
            tokio::task::spawn(async move {
                m.write().await.update_log(&f, &l, |log| {
                    log.streamed_text.push_str(&s);
                    log.status = LogStatus::Streaming {
                        tokens: log.streamed_text.clone(),
                    };
                });
            });
        };

        match chat_stream(config, SYSTEM_PROMPT, &full, ot).await {
            Ok(result) => match parse_llm_output(&result.text) {
                Ok(parsed) => {
                    retry_ctx.mark_success(&result.usage).await;
                    return Ok((result, parsed));
                }
                Err(e) => {
                    retry_ctx.record_failure(e).await;
                    if !retry_ctx.should_retry() {
                        return Err(retry_ctx.take_last_error().unwrap());
                    }
                    retry_ctx.wait().await;
                }
            },
            Err(e) => {
                retry_ctx.record_failure(e).await;
                if !retry_ctx.should_retry() {
                    return Err(retry_ctx.take_last_error().unwrap());
                }
                retry_ctx.wait().await;
            }
        }
    }
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
