use axum::{routing::get, Router};
use eyre::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Notify;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::agent_env::session::Session;

use super::web_ui::WebUI;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Session>,
    pub web_ui: Arc<WebUI>,
}

/// Web server for serving WebUI
pub struct WebServer {
    addr: SocketAddr,
    state: AppState,
}

impl WebServer {
    /// Create new web server
    pub fn new(addr: SocketAddr, session: Arc<Session>, web_ui: Arc<WebUI>) -> Self {
        Self {
            addr,
            state: AppState { session, web_ui },
        }
    }

    /// Build router with all routes
    fn build_router(&self) -> Router {
        Router::new()
            // WebSocket endpoint (placeholder for Phase 2)
            // .route("/ws/worker/:worker_id", get(super::websocket::websocket_handler))
            // REST API endpoints (placeholder for Phase 1.5)
            .route("/api/health", get(super::api::health_check))
            // .route("/api/workers", get(super::api::list_workers))
            // .route("/api/workers/:id", get(super::api::get_worker))
            // Static file serving (frontend)
            .nest_service("/", ServeDir::new("web/public"))
            // CORS for development
            .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
            // Shared state
            .with_state(self.state.clone())
    }

    /// Run the web server (blocks until shutdown)
    pub async fn run(self) -> Result<()> {
        let router = self.build_router();

        tracing::info!("Web server listening on http://{}", self.addr);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;

        axum::serve(listener, router).await?;

        Ok(())
    }

    /// Run with graceful shutdown signal
    pub async fn run_with_shutdown(self, shutdown_signal: Arc<Notify>) -> Result<()> {
        let router = self.build_router();

        tracing::info!("Web server listening on http://{}", self.addr);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;

        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown_signal.notified().await;
                tracing::info!("Web server shutting down");

                // Give connections 5 seconds to close
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            })
            .await?;

        Ok(())
    }
}
