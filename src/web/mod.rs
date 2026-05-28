pub mod api;
pub mod panel;
pub mod rss_route;

use axum::Router;
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn router(config: Arc<RwLock<crate::config::Config>>) -> Router {
    Router::new()
        .merge(panel::router())
        .merge(api::router(config.clone()))
        .merge(rss_route::router(config))
}
