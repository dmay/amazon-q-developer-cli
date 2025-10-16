use std::sync::Arc;

use crate::agent_env::session::Session;

/// WebUI component for broadcasting events to WebSocket clients
pub struct WebUI {
    _session: Arc<Session>,
}

impl WebUI {
    /// Create new WebUI
    pub fn new(session: Arc<Session>) -> Self {
        Self { _session: session }
    }
}
