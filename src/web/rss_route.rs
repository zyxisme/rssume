use super::api::AppState;
use axum::{Extension, Router, routing::get};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/feeds/{name}", get(serve_feed))
        .layer(Extension(state))
}

async fn serve_feed(
    axum::extract::Path(name): axum::extract::Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<(axum::http::StatusCode, [(String, String); 1], String), crate::error::AppError> {
    let config = state.config.read().await;
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
