use chrono::Utc;
use feed_rs::parser;

#[derive(Debug, Clone)]
pub struct RawArticle {
    pub title: String,
    pub link: String,
    pub content: String,
    pub published_at: String,
}

pub async fn fetch_feed(url: &str) -> Result<Vec<RawArticle>, crate::error::AppError> {
    let client = reqwest::Client::builder()
        .user_agent("rssume/0.1 (RSS middleware; +https://github.com/rssume/rssume)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| crate::error::AppError::Fetch(format!("Failed to build client: {}", e)))?;

    let resp =
        client.get(url).send().await.map_err(|e| {
            crate::error::AppError::Fetch(format!("Failed to fetch {}: {}", url, e))
        })?;

    if !resp.status().is_success() {
        return Err(crate::error::AppError::Fetch(format!(
            "HTTP {} when fetching {}",
            resp.status(),
            url
        )));
    }

    let body = resp.bytes().await.map_err(|e| {
        crate::error::AppError::Fetch(format!("Failed to read response body: {}", e))
    })?;

    let feed = parser::parse(&body[..])
        .map_err(|e| crate::error::AppError::Parse(format!("Failed to parse feed: {}", e)))?;

    let articles: Vec<RawArticle> = feed
        .entries
        .into_iter()
        .map(|entry| {
            let content = entry
                .content
                .as_ref()
                .and_then(|c| c.body.as_ref())
                .cloned()
                .unwrap_or_default();
            let summary = entry
                .summary
                .as_ref()
                .map(|s| s.content.clone())
                .unwrap_or_default();
            let body = if !content.is_empty() {
                content
            } else {
                summary
            };

            RawArticle {
                title: entry
                    .title
                    .as_ref()
                    .map(|t| t.content.clone())
                    .unwrap_or_default(),
                link: entry
                    .links
                    .first()
                    .map(|l| l.href.clone())
                    .unwrap_or_default(),
                content: body,
                published_at: entry
                    .published
                    .or(entry.updated)
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| Utc::now().to_string()),
            }
        })
        .filter(|a| !a.link.is_empty() && !a.title.is_empty())
        .collect();

    Ok(articles)
}
