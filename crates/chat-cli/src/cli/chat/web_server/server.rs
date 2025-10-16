use std::net::SocketAddr;
use std::sync::Arc;

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
    _addr: SocketAddr,
    _state: AppState,
}

impl WebServer {
    /// Create new web server
    pub fn new(
        addr: SocketAddr,
        session: Arc<Session>,
        web_ui: Arc<WebUI>,
    ) -> Self {
        Self {
            _addr: addr,
            _state: AppState { session, web_ui },
        }
    }
}
