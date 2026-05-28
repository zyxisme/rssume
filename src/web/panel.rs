use super::api::AppState;
use crate::monitor::LogStatus;
use axum::response::Html;
use axum::{Extension, Router, routing::get};
use std::sync::Arc;
use tera::{Context, Tera};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/panel", get(dashboard))
        .route("/panel/feed/{name}", get(feed_detail))
        .route("/panel/settings", get(settings))
        .route("/panel/monitor", get(monitor_page))
        .route("/panel/feed/{name}/logs", get(feed_logs_page))
        .route("/panel/feed/{name}/monitor", get(feed_monitor_page))
        .layer(Extension(state))
}

pub(super) fn tera_instance() -> Result<Tera, crate::error::AppError> {
    let mut tera = Tera::default();
    tera.add_raw_template("base.html", include_str!("../../templates/base.html"))?;
    tera.add_raw_template(
        "dashboard.html",
        include_str!("../../templates/dashboard.html"),
    )?;
    tera.add_raw_template("feed.html", include_str!("../../templates/feed.html"))?;
    tera.add_raw_template(
        "settings.html",
        include_str!("../../templates/settings.html"),
    )?;
    tera.add_raw_template("monitor.html", include_str!("../../templates/monitor.html"))?;
    tera.add_raw_template("logs.html", include_str!("../../templates/logs.html"))?;
    tera.add_raw_template(
        "feed_monitor.html",
        include_str!("../../templates/feed_monitor.html"),
    )?;
    tera.add_raw_template(
        "partials/stats_bar.html",
        include_str!("../../templates/partials/stats_bar.html"),
    )?;
    tera.add_raw_template(
        "partials/monitor_status.html",
        include_str!("../../templates/partials/monitor_status.html"),
    )?;
    tera.add_raw_template(
        "partials/feed_monitor_status.html",
        include_str!("../../templates/partials/feed_monitor_status.html"),
    )?;
    Ok(tera)
}

async fn dashboard(
    Extension(s): Extension<Arc<AppState>>,
) -> Result<Html<String>, crate::error::AppError> {
    let stats = crate::storage::all_feed_stats()?;
    let cfg = s.config.read().await;
    let mon = s.monitor.read().await;
    let tera = tera_instance()?;
    let feed_rows: Vec<serde_json::Value> = cfg
        .feeds
        .iter()
        .map(|f| {
            let rt = mon.feeds.get(&f.name);
            let s = stats.iter().find(|s| s.feed_name == f.name);
            serde_json::json!({
                "name": f.name,
                "url": f.url,
                "enabled": f.enabled,
                "status": format!("{:?}", rt.map(|r| &r.status).unwrap_or(&crate::monitor::FeedStatus::Idle)),
                "last_fetch_at": rt.and_then(|r| r.last_fetch_at.as_ref()),
                "article_count": s.map(|x| x.article_count).unwrap_or(0),
                "translated_count": s.map(|x| x.translated_count).unwrap_or(0),
                "with_summary_count": s.map(|x| x.with_summary_count).unwrap_or(0),
            })
        })
        .collect();
    let total_articles: usize = stats.iter().map(|x| x.article_count).sum();
    let total_translated: usize = stats.iter().map(|x| x.translated_count).sum();
    let total_with_summary: usize = stats.iter().map(|x| x.with_summary_count).sum();
    let mut ctx = Context::new();
    ctx.insert("title", "rssume Dashboard");
    ctx.insert("feeds_count", &cfg.feeds.len());
    ctx.insert("total_articles", &total_articles);
    ctx.insert("total_translated", &total_translated);
    ctx.insert("total_with_summary", &total_with_summary);
    ctx.insert("total_prompt_tokens", &mon.token_usage.total_prompt_tokens);
    ctx.insert(
        "total_completion_tokens",
        &mon.token_usage.total_completion_tokens,
    );
    ctx.insert("feed_rows", &feed_rows);
    Ok(Html(tera.render("dashboard.html", &ctx).map_err(|e| {
        crate::error::AppError::Storage(format!("render: {}", e))
    })?))
}

async fn feed_detail(
    Extension(s): Extension<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Html<String>, crate::error::AppError> {
    let data = crate::storage::FeedData::load(&name)?;
    let mon = s.monitor.read().await;
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", &format!("rssume - {}", name));
    ctx.insert("feed_name", &name);
    ctx.insert("articles", &data.articles);
    ctx.insert("runtime_status", &mon.feeds.get(&name).map(|s| &s.status));
    Ok(Html(tera.render("feed.html", &ctx).map_err(|e| {
        crate::error::AppError::Storage(format!("render: {}", e))
    })?))
}

async fn monitor_page(
    Extension(s): Extension<Arc<AppState>>,
) -> Result<Html<String>, crate::error::AppError> {
    let cfg = s.config.read().await;
    let mon = s.monitor.read().await;
    let tera = tera_instance()?;
    let active = mon.active_translations();
    let recent_count: usize = mon
        .translation_logs
        .values()
        .map(|l| {
            l.iter()
                .filter(|l| {
                    matches!(
                        l.status,
                        crate::monitor::LogStatus::Completed | crate::monitor::LogStatus::Failed(_)
                    )
                })
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
                "translating_current": match rt.map(|r| &r.status) {
                    Some(crate::monitor::FeedStatus::Translating { current, .. }) => current,
                    _ => &0u32,
                },
                "translating_total": match rt.map(|r| &r.status) {
                    Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
                    _ => &0u32,
                },
                "translating_title": match rt.map(|r| &r.status) {
                    Some(crate::monitor::FeedStatus::Translating { current_title, .. }) => current_title,
                    _ => "",
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
    let mut ctx = Context::new();
    ctx.insert("title", "rssume Monitor");
    ctx.insert("feeds", &feeds);
    ctx.insert("active", &active_translations);
    ctx.insert("recent_count", &recent_count);
    Ok(Html(tera.render("monitor.html", &ctx).map_err(|e| {
        crate::error::AppError::Storage(format!("render: {}", e))
    })?))
}

async fn feed_logs_page(
    Extension(s): Extension<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Html<String>, crate::error::AppError> {
    let mon = s.monitor.read().await;
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", &format!("rssume - {} logs", name));
    ctx.insert("feed_name", &name);
    ctx.insert("logs", &mon.get_logs(&name));
    Ok(Html(tera.render("logs.html", &ctx).map_err(|e| {
        crate::error::AppError::Storage(format!("render: {}", e))
    })?))
}

async fn feed_monitor_page(
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
        "translating_current": match rt.map(|r| &r.status) {
            Some(crate::monitor::FeedStatus::Translating { current, .. }) => current,
            _ => &0u32,
        },
        "translating_total": match rt.map(|r| &r.status) {
            Some(crate::monitor::FeedStatus::Translating { total, .. }) => total,
            _ => &0u32,
        },
        "translating_title": match rt.map(|r| &r.status) {
            Some(crate::monitor::FeedStatus::Translating { current_title, .. }) => current_title,
            _ => "",
        },
    });
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", &format!("rssume - {}", name));
    ctx.insert("feed_name", &name);
    ctx.insert("feed", &feed);
    ctx.insert("active", &active);
    ctx.insert("recent_count", &recent_count);
    Ok(Html(tera.render("feed_monitor.html", &ctx).map_err(
        |e| crate::error::AppError::Storage(format!("render: {}", e)),
    )?))
}

async fn settings() -> Result<Html<String>, crate::error::AppError> {
    let config = crate::config::Config::load().unwrap_or_else(|_| crate::config::Config {
        server: crate::config::ServerConfig {
            host: "127.0.0.1".into(),
            port: 3000,
        },
        language: crate::config::LanguageConfig {
            target: "zh_CN".into(),
        },
        llm: crate::config::LlmConfig {
            translation: crate::config::LlmProviderConfig {
                provider: "".into(),
                model: "".into(),
                api_key: "".into(),
                base_url: "".into(),
                prompt_append: None,
                max_tokens: None,
            },
            summary: crate::config::LlmProviderConfig {
                provider: "".into(),
                model: "".into(),
                api_key: "".into(),
                base_url: "".into(),
                prompt_append: None,
                max_tokens: None,
            },
        },
        feeds: vec![],
        logging: Default::default(),
    });
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", "rssume Settings");
    ctx.insert("config", &config);
    Ok(Html(tera.render("settings.html", &ctx).map_err(|e| {
        crate::error::AppError::Storage(format!("render: {}", e))
    })?))
}
