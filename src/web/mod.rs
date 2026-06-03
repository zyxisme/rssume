pub mod api;
pub mod panel;
pub mod rss_route;

use api::AppState;
use axum::Router;
use axum::response::Redirect;
use std::sync::Arc;

pub fn router(
    config: Arc<tokio::sync::RwLock<crate::config::Config>>,
    monitor: Arc<tokio::sync::RwLock<crate::monitor::Monitor>>,
) -> Router {
    let state = Arc::new(AppState {
        config: config.clone(),
        monitor: monitor.clone(),
    });
    Router::new()
        .route(
            "/",
            axum::routing::get(|| async { Redirect::permanent("/panel") }),
        )
        .merge(panel::router(state.clone()))
        .merge(api::router(state.clone()))
        .merge(rss_route::router(state.clone()))
}
