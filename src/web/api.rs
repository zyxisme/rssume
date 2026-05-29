use crate::monitor::{LogStatus, Monitor};
use axum::response::{Html, IntoResponse};
use axum::{Extension, Json, Router, routing::get};
use serde::Serialize;
use std::sync::Arc;
use tera::Context;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<tokio::sync::RwLock<crate::config::Config>>,
    pub monitor: Arc<tokio::sync::RwLock<Monitor>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/feeds", get(list_feeds))
        .route("/api/feeds/export.opml", get(export_opml))
        .route("/api/monitor/status", get(monitor_status))
        .route("/api/monitor/translating", get(monitor_translating))
        .route(
            "/api/monitor/feed/{name}/translating",
            get(monitor_feed_translating),
        )
        .route("/api/monitor/logs/{name}", get(monitor_logs))
        .route("/api/token-usage", get(token_usage))
        .layer(Extension(state))
}

async fn get_stats(
    Extension(s): Extension<Arc<AppState>>,
) -> Result<Html<String>, crate::error::AppError> {
    let stats = crate::storage::all_feed_stats()?;
    let cfg = s.config.read().await;
    let tu = &s.monitor.read().await.token_usage;
    let tera = super::panel::tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("feeds_count", &cfg.feeds.len());
    ctx.insert(
        "total_articles",
        &stats.iter().map(|x| x.article_count).sum::<usize>(),
    );
    ctx.insert(
        "total_translated",
        &stats.iter().map(|x| x.translated_count).sum::<usize>(),
    );
    ctx.insert(
        "total_with_summary",
        &stats.iter().map(|x| x.with_summary_count).sum::<usize>(),
    );
    ctx.insert("total_prompt_tokens", &tu.total_prompt_tokens);
    ctx.insert("total_completion_tokens", &tu.total_completion_tokens);
    Ok(Html(tera.render("partials/stats_bar.html", &ctx).map_err(
        |e| crate::error::AppError::Storage(format!("render: {}", e)),
    )?))
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

async fn export_opml(Extension(s): Extension<Arc<AppState>>) -> impl IntoResponse {
    let cfg = s.config.read().await;
    let opml = crate::opml::generate_opml(&cfg.feeds);

    (
        [
            ("Content-Type", "application/xml; charset=utf-8"),
            (
                "Content-Disposition",
                r#"attachment; filename="rssume-subscriptions.opml""#,
            ),
        ],
        opml,
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

async fn monitor_translating(
    Extension(s): Extension<Arc<AppState>>,
) -> Result<Html<String>, crate::error::AppError> {
    let mon = s.monitor.read().await;
    let cfg = s.config.read().await;
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
    let feeds: Vec<serde_json::Value> = cfg
        .feeds
        .iter()
        .map(|f| {
            let rt = mon.feeds.get(&f.name);
            let d = crate::storage::FeedData::load(&f.name).ok();
            serde_json::json!({
                "name": f.name,
                "status": rt.map(|r| format!("{:?}", r.status)).unwrap_or_else(|| "Idle".into()),
                "articles": d.as_ref().map(|d| d.article_count()).unwrap_or(0),
                "last_fetch_at": rt.and_then(|r| r.last_fetch_at.as_ref()),
                "translating_completed": match rt.map(|r| &r.status) {
                    Some(crate::monitor::FeedStatus::Translating { completed, .. }) => completed,
                    _ => &0u32,
                },
                "translating_total": match rt.map(|r| &r.status) {
                    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
                    _ => &0u32,
                },
                "translating_in_progress": match rt.map(|r| &r.status) {
                    Some(crate::monitor::FeedStatus::Translating {
                        in_progress, ..
                    }) => in_progress.clone(),
                    _ => vec![],
                },
            })
        })
        .collect();
    let active_translations: Vec<serde_json::Value> = active
        .iter()
        .map(|(f, l)| {
            serde_json::json!({
                "feed_name": f,
                "article_title": l.article_title,
                "stage": format!("{:?}", l.stage),
                "streamed_text": match &l.status {
                    crate::monitor::LogStatus::Streaming { tokens } => tokens.clone(),
                    _ => String::new(),
                },
            })
        })
        .collect();
    let tera = super::panel::tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("feeds", &feeds);
    ctx.insert("active", &active_translations);
    ctx.insert("recent_count", &recent_count);
    Ok(Html(
        tera.render("partials/monitor_status.html", &ctx)
            .map_err(|e| crate::error::AppError::Storage(format!("render: {}", e)))?,
    ))
}

async fn monitor_feed_translating(
    Extension(s): Extension<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Html<String>, crate::error::AppError> {
    let mon = s.monitor.read().await;
    let rt = mon.feeds.get(&name);
    let d = crate::storage::FeedData::load(&name).ok();
    let active: Vec<serde_json::Value> = mon
        .translation_logs
        .get(&name)
        .map(|logs| {
            logs.iter()
                .rev()
                .filter(|l| matches!(l.status, LogStatus::Started | LogStatus::Streaming { .. }))
                .map(|l| {
                    serde_json::json!({
                        "article_title": l.article_title,
                        "stage": format!("{:?}", l.stage),
                        "model": l.model,
                        "streamed_text": match &l.status {
                            LogStatus::Streaming { tokens } => tokens.clone(),
                            _ => String::new(),
                        },
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let recent_count: usize = mon
        .translation_logs
        .get(&name)
        .map(|logs| {
            logs.iter()
                .filter(|l| matches!(l.status, LogStatus::Completed | LogStatus::Failed(_)))
                .count()
        })
        .unwrap_or(0);
    let feed = serde_json::json!({
        "name": name,
        "status": rt.map(|r| format!("{:?}", r.status)).unwrap_or_else(|| "Idle".into()),
        "articles": d.as_ref().map(|d| d.article_count()).unwrap_or(0),
        "translated": d.as_ref().map(|d| d.translated_count()).unwrap_or(0),
        "last_fetch_at": rt.and_then(|r| r.last_fetch_at.as_ref()),
        "last_fetch_error": rt.and_then(|r| r.last_fetch_error.as_ref()),
        "translating_completed": match rt.map(|r| &r.status) {
            Some(crate::monitor::FeedStatus::Translating { completed, .. }) => completed,
            _ => &0u32,
        },
        "translating_total": match rt.map(|r| &r.status) {
            Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
            _ => &0u32,
        },
        "translating_in_progress": match rt.map(|r| &r.status) {
            Some(crate::monitor::FeedStatus::Translating {
                in_progress, ..
            }) => in_progress.clone(),
            _ => vec![],
        },
    });
    let tera = super::panel::tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("feed", &feed);
    ctx.insert("active", &active);
    ctx.insert("recent_count", &recent_count);
    Ok(Html(
        tera.render("partials/feed_monitor_status.html", &ctx)
            .map_err(|e| crate::error::AppError::Storage(format!("render: {}", e)))?,
    ))
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
