use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn router(config: Arc<RwLock<crate::config::Config>>) -> Router {
    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/feeds", get(list_feeds))
        .with_state(config)
}

#[derive(Serialize)]
struct ApiStats {
    feeds: Vec<crate::storage::FeedStats>,
    total_articles: usize,
    total_translated: usize,
    total_with_summary: usize,
}

async fn get_stats(
    State(_config): State<Arc<RwLock<crate::config::Config>>>,
) -> Result<Json<ApiStats>, crate::error::AppError> {
    let stats = crate::storage::all_feed_stats()?;
    let total_articles: usize = stats.iter().map(|s| s.article_count).sum();
    let total_translated: usize = stats.iter().map(|s| s.translated_count).sum();
    let total_with_summary: usize = stats.iter().map(|s| s.with_summary_count).sum();

    Ok(Json(ApiStats {
        feeds: stats,
        total_articles,
        total_translated,
        total_with_summary,
    }))
}

#[derive(Serialize)]
struct FeedInfo {
    name: String,
    url: String,
    enabled: bool,
    interval_secs: u64,
}

async fn list_feeds(
    State(config): State<Arc<RwLock<crate::config::Config>>>,
) -> Json<Vec<FeedInfo>> {
    let config = config.read().await;
    let feeds: Vec<FeedInfo> = config
        .feeds
        .iter()
        .map(|f| FeedInfo {
            name: f.name.clone(),
            url: f.url.clone(),
            enabled: f.enabled,
            interval_secs: f.interval_secs,
        })
        .collect();
    Json(feeds)
}
