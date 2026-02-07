use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;

use crate::db::WebhookDb;
use crate::webhooks;

/// Maximum events buffered per board channel before old events are dropped.
const CHANNEL_CAPACITY: usize = 256;

/// A board-level event broadcast system.
///
/// Each board gets its own broadcast channel, created lazily on first
/// subscription. Events are sent to all subscribers of a board.
/// Also delivers events to registered webhooks.
pub struct EventBus {
    channels: Mutex<HashMap<String, broadcast::Sender<BoardEvent>>>,
    webhook_db: Option<WebhookDb>,
    http_client: reqwest::Client,
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
        Self::new()
    }
}

impl EventBus {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
            webhook_db: None,
            http_client: reqwest::Client::new(),
        }
    }

    /// Create an EventBus with webhook delivery support.
    pub fn with_webhooks(webhook_db: WebhookDb) -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
            webhook_db: Some(webhook_db),
            http_client: reqwest::Client::new(),
        }
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
    /// Also delivers to registered webhooks asynchronously.
    pub fn emit(&self, event: BoardEvent) {
        // Deliver to SSE subscribers
        let channels = self.channels.lock().unwrap();
        if let Some(sender) = channels.get(&event.board_id) {
            // Ignore send errors (no subscribers)
            let _ = sender.send(event.clone());
        }
        drop(channels);

        // Deliver to webhooks (async, non-blocking)
        if let Some(ref db) = self.webhook_db {
            webhooks::deliver_webhooks(db.clone(), event, self.http_client.clone());
        }
    }
}
