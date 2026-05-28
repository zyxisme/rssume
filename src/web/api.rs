use crate::monitor::{LogStatus, Monitor};
use axum::{Extension, Json, Router, routing::get};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<tokio::sync::RwLock<crate::config::Config>>,
    pub monitor: Arc<tokio::sync::RwLock<Monitor>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/feeds", get(list_feeds))
        .route("/api/monitor/status", get(monitor_status))
        .route("/api/monitor/translating", get(monitor_translating))
        .route("/api/monitor/logs/{name}", get(monitor_logs))
        .route("/api/token-usage", get(token_usage))
        .layer(Extension(state))
}

#[derive(Serialize)]
struct ApiStats {
    feeds: Vec<crate::storage::FeedStats>,
    total_articles: usize,
    total_translated: usize,
    total_with_summary: usize,
    total_prompt_tokens: u64,
    total_completion_tokens: u64,
}

async fn get_stats(
    Extension(s): Extension<Arc<AppState>>,
) -> Result<Json<ApiStats>, crate::error::AppError> {
    let stats = crate::storage::all_feed_stats()?;
    let tu = &s.monitor.read().await.token_usage;
    Ok(Json(ApiStats {
        total_articles: stats.iter().map(|x| x.article_count).sum(),
        total_translated: stats.iter().map(|x| x.translated_count).sum(),
        total_with_summary: stats.iter().map(|x| x.with_summary_count).sum(),
        total_prompt_tokens: tu.total_prompt_tokens,
        total_completion_tokens: tu.total_completion_tokens,
        feeds: stats,
    }))
}

#[derive(Serialize)]
struct FeedInfo {
    name: String,
    url: String,
    enabled: bool,
    interval_secs: u64,
}

async fn list_feeds(Extension(s): Extension<Arc<AppState>>) -> Json<Vec<FeedInfo>> {
    let c = s.config.read().await;
    Json(
        c.feeds
            .iter()
            .map(|f| FeedInfo {
                name: f.name.clone(),
                url: f.url.clone(),
                enabled: f.enabled,
                interval_secs: f.interval_secs,
            })
            .collect(),
    )
}

async fn monitor_status(Extension(s): Extension<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    let mon = s.monitor.read().await;
    let cfg = s.config.read().await;
    Json(
        cfg.feeds
            .iter()
            .map(|f| {
                let rt = mon.feeds.get(&f.name);
                let d = crate::storage::FeedData::load(&f.name).ok();
                serde_json::json!({
                    "name": f.name,
                    "url": f.url,
                    "enabled": f.enabled,
                    "status": rt.map(|r| &r.status),
                    "last_fetch_at": rt.and_then(|r| r.last_fetch_at.as_ref()),
                    "last_fetch_error": rt.and_then(|r| r.last_fetch_error.as_ref()),
                    "last_poll_duration_ms": rt.map(|r| r.last_poll_duration_ms).unwrap_or(0),
                    "articles": d.as_ref().map(|d| d.article_count()).unwrap_or(0),
                    "translated": d.as_ref().map(|d| d.translated_count()).unwrap_or(0),
                    "summarized": d.as_ref().map(|d| d.with_summary_count()).unwrap_or(0),
                })
            })
            .collect(),
    )
}

async fn monitor_translating(Extension(s): Extension<Arc<AppState>>) -> Json<serde_json::Value> {
    let mon = s.monitor.read().await;
    let cfg = s.config.read().await;
    let feeds_status: Vec<_> = cfg
        .feeds
        .iter()
        .filter_map(|f| {
            mon.feeds
                .get(&f.name)
                .map(|s| (f.name.clone(), s.status.clone()))
        })
        .collect();
    let active = mon.active_translations();
    let recent_count: usize = mon
        .translation_logs
        .values()
        .map(|l| {
            l.iter()
                .filter(|l| matches!(l.status, LogStatus::Completed | LogStatus::Failed(_)))
                .count()
        })
        .sum();
    Json(serde_json::json!({
        "feeds_status": feeds_status,
        "active": active.iter().map(|(f, l)| serde_json::json!({
            "feed_name": f,
            "log": l,
        })).collect::<Vec<_>>(),
        "recent_count": recent_count,
    }))
}

async fn monitor_logs(
    Extension(s): Extension<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Json<Vec<crate::monitor::TranslationLog>> {
    Json(
        s.monitor
            .read()
            .await
            .get_logs(&name)
            .into_iter()
            .cloned()
            .collect(),
    )
}

async fn token_usage(Extension(s): Extension<Arc<AppState>>) -> Json<crate::monitor::TokenUsage> {
    Json(s.monitor.read().await.token_usage.clone())
}
