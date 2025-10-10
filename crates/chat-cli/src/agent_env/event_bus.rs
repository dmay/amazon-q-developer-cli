//! Central event distribution system using tokio broadcast channels.

use tokio::sync::broadcast;
use super::events::AgentEnvironmentEvent;

/// Central event distribution system
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<AgentEnvironmentEvent>,
    buffer_size: usize,
}

impl EventBus {
    /// Create new EventBus with specified buffer size
    pub fn new(buffer_size: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer_size);
        Self { sender, buffer_size }
    }

    /// Publish event to all subscribers
    pub fn publish(&self, event: AgentEnvironmentEvent) {
        let _ = self.sender.send(event);
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEnvironmentEvent> {
        self.sender.subscribe()
    }

    /// Get current subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1000)
    }
}
