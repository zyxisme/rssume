use axum::{Router, extract::State, routing::get};
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn router(config: Arc<RwLock<crate::config::Config>>) -> Router {
    Router::new()
        .route("/feeds/{name}", get(serve_feed))
        .with_state(config)
}

async fn serve_feed(
    axum::extract::Path(name): axum::extract::Path<String>,
    State(config): State<Arc<RwLock<crate::config::Config>>>,
) -> Result<(axum::http::StatusCode, [(String, String); 1], String), crate::error::AppError> {
    let config = config.read().await;
    let feed_cfg = config
        .feeds
        .iter()
        .find(|f| f.name == name)
        .ok_or_else(|| crate::error::AppError::NotFound(format!("Feed '{}' not found", name)))?;

    if !feed_cfg.enabled {
        return Err(crate::error::AppError::NotFound(format!(
            "Feed '{}' is disabled",
            name
        )));
    }

    let feed_data = crate::storage::FeedData::load(&name)?;
    let rss_xml = crate::rss::generate::generate_rss(&name, &feed_data.articles);

    Ok((
        axum::http::StatusCode::OK,
        [(
            "Content-Type".to_string(),
            "application/rss+xml; charset=utf-8".to_string(),
        )],
        rss_xml,
    ))
}
