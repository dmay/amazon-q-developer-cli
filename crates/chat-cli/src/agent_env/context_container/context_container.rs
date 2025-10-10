use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use super::conversation_history::ConversationHistory;

/// Container for all contextual information available to a worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextContainer {
    #[serde(skip)]
    pub conversation_history: Arc<Mutex<ConversationHistory>>,
}

impl ContextContainer {
    pub fn new() -> Self {
        Self {
            conversation_history: Arc::new(Mutex::new(ConversationHistory::new())),
        }
    }
}

impl Default for ContextContainer {
    fn default() -> Self {
        Self::new()
    }
}
