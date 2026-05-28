use axum::response::Html;
use axum::{Router, routing::get};
use tera::Context;

pub fn router() -> Router {
    Router::new()
        .route("/panel", get(dashboard))
        .route("/panel/feed/:name", get(feed_detail))
        .route("/panel/settings", get(settings))
}

async fn dashboard() -> Result<Html<String>, crate::error::AppError> {
    let stats = crate::storage::all_feed_stats()?;
    let config = crate::config::Config::load().unwrap_or_else(|_| crate::config::Config {
        server: crate::config::ServerConfig {
            host: "127.0.0.1".into(),
            port: 3000,
        },
        language: crate::config::LanguageConfig {
            target: "zho".into(),
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

    let tera = tera::Tera::new("templates/**/*.html")
        .map_err(|e| crate::error::AppError::Storage(format!("Template error: {}", e)))?;

    let mut ctx = Context::new();
    ctx.insert("title", "rssume Dashboard");
    ctx.insert("feeds", &config.feeds);
    ctx.insert("stats", &stats);

    let html = tera
        .render("dashboard.html", &ctx)
        .map_err(|e| crate::error::AppError::Storage(format!("Render error: {}", e)))?;

    Ok(Html(html))
}

async fn feed_detail(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Html<String>, crate::error::AppError> {
    let feed_data = crate::storage::FeedData::load(&name)?;

    let tera = tera::Tera::new("templates/**/*.html")
        .map_err(|e| crate::error::AppError::Storage(format!("Template error: {}", e)))?;

    let mut ctx = Context::new();
    ctx.insert("title", &format!("rssume - {}", name));
    ctx.insert("feed_name", &name);
    ctx.insert("articles", &feed_data.articles);

    let html = tera
        .render("feed.html", &ctx)
        .map_err(|e| crate::error::AppError::Storage(format!("Render error: {}", e)))?;

    Ok(Html(html))
}

async fn settings() -> Result<Html<String>, crate::error::AppError> {
    let config = crate::config::Config::load().unwrap_or_else(|_| crate::config::Config {
        server: crate::config::ServerConfig {
            host: "127.0.0.1".into(),
            port: 3000,
        },
        language: crate::config::LanguageConfig {
            target: "zho".into(),
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

    let tera = tera::Tera::new("templates/**/*.html")
        .map_err(|e| crate::error::AppError::Storage(format!("Template error: {}", e)))?;

    let mut ctx = Context::new();
    ctx.insert("title", "rssume Settings");
    ctx.insert("config", &config);

    let html = tera
        .render("settings.html", &ctx)
        .map_err(|e| crate::error::AppError::Storage(format!("Render error: {}", e)))?;

    Ok(Html(html))
}
