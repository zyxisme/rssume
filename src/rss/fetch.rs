use chrono::Utc;
use feed_rs::parser;

#[derive(Debug, Clone)]
pub struct RawArticle {
    pub title: String,
    pub link: String,
    pub content: String,
    pub published_at: String,
    pub author: Option<String>,
    pub categories: Vec<String>,
    pub guid: Option<String>,
    pub media_urls: Vec<MediaItem>,
}

#[derive(Debug, Clone)]
pub struct MediaItem {
    pub url: String,
    pub content_type: Option<String>,
    pub length: Option<u64>,
}

pub async fn fetch_feed(url: &str) -> Result<Vec<RawArticle>, crate::error::AppError> {
    let client = reqwest::Client::builder()
        .user_agent("rssume/0.1 (RSS middleware; +https://github.com/rssume/rssume)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| crate::error::AppError::Fetch(format!("client: {}", e)))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| crate::error::AppError::Fetch(format!("fetch {}: {}", url, e)))?;
    if !resp.status().is_success() {
        return Err(crate::error::AppError::Fetch(format!(
            "HTTP {} for {}",
            resp.status(),
            url
        )));
    }

    let body = resp
        .bytes()
        .await
        .map_err(|e| crate::error::AppError::Fetch(format!("body: {}", e)))?;
    let feed = parser::parse(&body[..])
        .map_err(|e| crate::error::AppError::Parse(format!("parse: {}", e)))?;

    let articles: Vec<_> = feed
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
            let media_urls = entry
                .media
                .iter()
                .flat_map(|m| &m.content)
                .filter_map(|mc| {
                    mc.url.as_ref().map(|u| MediaItem {
                        url: u.to_string(),
                        content_type: mc.content_type.as_ref().map(|t| t.to_string()),
                        length: mc.size,
                    })
                })
                .collect();

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
                    .map(|d| d.to_rfc2822())
                    .unwrap_or_else(|| Utc::now().to_rfc2822()),
                author: entry.authors.first().map(|p| p.name.clone()),
                categories: entry.categories.into_iter().map(|c| c.term).collect(),
                guid: Some(entry.id),
                media_urls,
            }
        })
        .filter(|a| !a.link.is_empty() && !a.title.is_empty())
        .collect();

    Ok(articles)
}
