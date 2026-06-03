use super::api::AppState;
use axum::{Extension, Router, routing::get};
use std::sync::Arc;

const RSS_STYLE_XSL: &str = include_str!("../../templates/rss_style.xsl");
const HIGHLIGHT_JS: &[u8] = include_bytes!("../../static/assets/highlight.min.js");
const HIGHLIGHT_CSS: &[u8] = include_bytes!("../../static/assets/highlight-github.min.css");
const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../static/assets/JetBrainsMono-Regular.woff2");
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("../../static/assets/JetBrainsMono-Bold.woff2");

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/feeds/{name}", get(serve_feed))
        .route("/feeds/style.xsl", get(serve_style))
        .route("/feeds/assets/highlight.min.js", get(serve_highlight_js))
        .route("/feeds/assets/highlight.min.css", get(serve_highlight_css))
        .route(
            "/feeds/assets/jetbrains-mono-regular.woff2",
            get(serve_jetbrains_mono_regular),
        )
        .route(
            "/feeds/assets/jetbrains-mono-bold.woff2",
            get(serve_jetbrains_mono_bold),
        )
        .layer(Extension(state))
}

async fn serve_style() -> (axum::http::StatusCode, [(String, String); 1], String) {
    (
        axum::http::StatusCode::OK,
        [(
            "Content-Type".to_string(),
            "application/xslt+xml; charset=utf-8".to_string(),
        )],
        RSS_STYLE_XSL.to_string(),
    )
}

async fn serve_highlight_js() -> (axum::http::StatusCode, [(String, String); 2], Vec<u8>) {
    (
        axum::http::StatusCode::OK,
        [
            (
                "Content-Type".to_string(),
                "application/javascript; charset=utf-8".to_string(),
            ),
            (
                "Cache-Control".to_string(),
                "public, max-age=31536000".to_string(),
            ),
        ],
        HIGHLIGHT_JS.to_vec(),
    )
}

async fn serve_highlight_css() -> (axum::http::StatusCode, [(String, String); 2], Vec<u8>) {
    (
        axum::http::StatusCode::OK,
        [
            (
                "Content-Type".to_string(),
                "text/css; charset=utf-8".to_string(),
            ),
            (
                "Cache-Control".to_string(),
                "public, max-age=31536000".to_string(),
            ),
        ],
        HIGHLIGHT_CSS.to_vec(),
    )
}

async fn serve_jetbrains_mono_regular() -> (axum::http::StatusCode, [(String, String); 2], Vec<u8>)
{
    (
        axum::http::StatusCode::OK,
        [
            ("Content-Type".to_string(), "font/woff2".to_string()),
            (
                "Cache-Control".to_string(),
                "public, max-age=31536000".to_string(),
            ),
        ],
        JETBRAINS_MONO_REGULAR.to_vec(),
    )
}

async fn serve_jetbrains_mono_bold() -> (axum::http::StatusCode, [(String, String); 2], Vec<u8>) {
    (
        axum::http::StatusCode::OK,
        [
            ("Content-Type".to_string(), "font/woff2".to_string()),
            (
                "Cache-Control".to_string(),
                "public, max-age=31536000".to_string(),
            ),
        ],
        JETBRAINS_MONO_BOLD.to_vec(),
    )
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
            "text/xml; charset=utf-8".to_string(),
        )],
        rss_xml,
    ))
}
