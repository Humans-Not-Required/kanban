use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;

/// Maximum events buffered per board channel before old events are dropped.
const CHANNEL_CAPACITY: usize = 256;

/// A board-level event broadcast system.
///
/// Each board gets its own broadcast channel, created lazily on first
/// subscription. Events are sent to all subscribers of a board.
pub struct EventBus {
    channels: Mutex<HashMap<String, broadcast::Sender<BoardEvent>>>,
}

/// A typed event emitted when something happens on a board.
#[derive(Debug, Clone, Serialize)]
pub struct BoardEvent {
    /// The type of event (e.g., "task.created", "task.moved")
    pub event: String,
    /// Board ID this event belongs to
    pub board_id: String,
    /// JSON payload with event-specific data
    pub data: serde_json::Value,
}

impl Default for EventBus {
    fn default() -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
        }
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to events for a specific board.
    /// Returns a broadcast receiver that yields BoardEvents.
    pub fn subscribe(&self, board_id: &str) -> broadcast::Receiver<BoardEvent> {
        let mut channels = self.channels.lock().unwrap();
        let sender = channels
            .entry(board_id.to_string())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0);
        sender.subscribe()
    }

    /// Emit an event to all subscribers of a board.
    /// Silently does nothing if no one is subscribed.
    pub fn emit(&self, event: BoardEvent) {
        let channels = self.channels.lock().unwrap();
        if let Some(sender) = channels.get(&event.board_id) {
            // Ignore send errors (no subscribers)
            let _ = sender.send(event);
        }
    }
}
