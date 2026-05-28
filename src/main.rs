mod config;
mod error;
mod lang;
mod llm;
mod monitor;
mod rss;
mod scheduler;
mod storage;
mod web;

use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = match config::Config::load() {
        Ok(c) => {
            tracing::info!("Config loaded from {}", config::config_path().display());
            c
        }
        Err(e) => {
            tracing::error!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    let data_dir = config::Config::data_dir();
    std::fs::create_dir_all(&data_dir).ok();
    tracing::info!("Data directory: {}", data_dir.display());

    let config = Arc::new(RwLock::new(config));

    let scheduler = Arc::new(scheduler::Scheduler::new(config.clone()));
    let scheduler_clone = scheduler.clone();
    tokio::spawn(async move {
        scheduler_clone.run_loop().await;
    });

    let app = web::router(config.clone());

    let host = config.read().await.server.host.clone();
    let port = config.read().await.server.port;
    let addr = format!("{}:{}", host, port);

    tracing::info!("rssume starting on http://{}", addr);
    tracing::info!("  Web panel:  http://{}/panel", addr);
    tracing::info!("  RSS feeds:  http://{}/feeds/{{feed_name}}", addr);
    tracing::info!("  API:        http://{}/api/stats", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
    tracing::info!("Shutting down...");
}
