use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::db::WebhookDb;
use crate::events::BoardEvent;

type HmacSha256 = Hmac<Sha256>;

/// Webhook metadata loaded from the database.
#[derive(Debug, Clone)]
struct WebhookTarget {
    id: String,
    url: String,
    secret: String,
    events: Vec<String>,
}

/// Compute HMAC-SHA256 signature for a payload.
fn sign_payload(secret: &str, payload: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload);
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Deliver a board event to all registered webhooks for that board.
/// Runs asynchronously — failures are logged and counted, not propagated.
pub fn deliver_webhooks(db: WebhookDb, event: BoardEvent, client: reqwest::Client) {
    tokio::spawn(async move {
        let targets = {
            let conn = db.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT id, url, secret, events FROM webhooks
                     WHERE board_id = ?1 AND active = 1 AND failure_count < 10",
                )
                .ok();

            match stmt {
                Some(ref mut s) => s
                    .query_map(rusqlite::params![event.board_id], |row| {
                        let events_str: String = row.get(3)?;
                        let events: Vec<String> =
                            serde_json::from_str(&events_str).unwrap_or_default();
                        Ok(WebhookTarget {
                            id: row.get(0)?,
                            url: row.get(1)?,
                            secret: row.get(2)?,
                            events,
                        })
                    })
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
                    .unwrap_or_default(),
                None => Vec::new(),
            }
        };

        if targets.is_empty() {
            return;
        }

        let payload = serde_json::json!({
            "event": event.event,
            "board_id": event.board_id,
            "data": event.data,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();

        for target in targets {
            // Filter: if webhook has specific events configured, check if this event matches
            if !target.events.is_empty() && !target.events.contains(&event.event) {
                continue;
            }

            let signature = sign_payload(&target.secret, &payload_bytes);

            let result = client
                .post(&target.url)
                .header("Content-Type", "application/json")
                .header("X-Kanban-Signature", format!("sha256={}", signature))
                .header("X-Kanban-Event", &event.event)
                .header("X-Kanban-Board", &event.board_id)
                .body(payload_bytes.clone())
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await;

            let success = match result {
                Ok(resp) => resp.status().is_success(),
                Err(_) => false,
            };

            // Update webhook stats in the database
            let db_ref = db.clone();
            let webhook_id = target.id.clone();
            if success {
                let conn = db_ref.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE webhooks SET failure_count = 0, last_triggered_at = datetime('now') WHERE id = ?1",
                    rusqlite::params![webhook_id],
                );
            } else {
                let conn = db_ref.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE webhooks SET failure_count = failure_count + 1, last_triggered_at = datetime('now') WHERE id = ?1",
                    rusqlite::params![webhook_id],
                );
            }
        }
    });
}
