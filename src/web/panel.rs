use super::api::AppState;
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
        .layer(Extension(state))
}

fn tera_instance() -> Result<Tera, crate::error::AppError> {
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
    Ok(tera)
}

async fn dashboard(
    Extension(s): Extension<Arc<AppState>>,
) -> Result<Html<String>, crate::error::AppError> {
    let stats = crate::storage::all_feed_stats()?;
    let cfg = s.config.read().await;
    let mon = s.monitor.read().await;
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", "rssume Dashboard");
    ctx.insert("feeds", &cfg.feeds);
    ctx.insert("stats", &stats);
    ctx.insert("total_prompt_tokens", &mon.token_usage.total_prompt_tokens);
    ctx.insert(
        "total_completion_tokens",
        &mon.token_usage.total_completion_tokens,
    );
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
    let tera = tera_instance()?;
    let mut ctx = Context::new();
    ctx.insert("title", "rssume Monitor");
    ctx.insert("feeds", &cfg.feeds);
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
            },
            summary: crate::config::LlmProviderConfig {
                provider: "".into(),
                model: "".into(),
                api_key: "".into(),
                base_url: "".into(),
                prompt_append: None,
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
