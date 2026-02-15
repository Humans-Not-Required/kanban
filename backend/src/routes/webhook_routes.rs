use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool};
use crate::models::*;

use super::{db_error, not_found};

/// Create a webhook — requires manage key.
#[post("/boards/<board_id>/webhooks", format = "json", data = "<req>")]
pub fn create_webhook(
    board_id: &str,
    req: Json<CreateWebhookRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<WebhookResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    if req.url.trim().is_empty() {
        return Err((Status::BadRequest, Json(ApiError {
            error: "Webhook URL cannot be empty".to_string(),
            code: "EMPTY_URL".to_string(),
            status: 400,
        })));
    }

    let valid_events = [
        "task.created", "task.updated", "task.deleted", "task.claimed",
        "task.released", "task.moved", "task.reordered", "task.comment",
        "task.archived", "task.unarchived", "task.dependency.added", "task.dependency.removed",
    ];
    for ev in &req.events {
        if !valid_events.contains(&ev.as_str()) {
            return Err((Status::BadRequest, Json(ApiError {
                error: format!("Invalid event type '{}'. Valid types: {}", ev, valid_events.join(", ")),
                code: "INVALID_EVENT_TYPE".to_string(),
                status: 400,
            })));
        }
    }

    let webhook_id = uuid::Uuid::new_v4().to_string();
    let secret = format!("whsec_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let events_json = serde_json::to_string(&req.events).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO webhooks (id, board_id, url, secret, events) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![webhook_id, board_id, req.url.trim(), secret, events_json],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    Ok(Json(WebhookResponse {
        id: webhook_id,
        board_id: board_id.to_string(),
        url: req.url,
        secret: Some(secret),
        events: req.events,
        active: true,
        failure_count: 0,
        last_triggered_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// List webhooks — requires manage key.
#[get("/boards/<board_id>/webhooks")]
pub fn list_webhooks(
    board_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<Vec<WebhookResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let mut stmt = conn.prepare(
        "SELECT id, board_id, url, events, active, failure_count, last_triggered_at, created_at FROM webhooks WHERE board_id = ?1 ORDER BY created_at ASC",
    ).map_err(|e| db_error(&e.to_string()))?;

    let webhooks: Vec<WebhookResponse> = stmt
        .query_map(rusqlite::params![board_id], |row| {
            let events_str: String = row.get(3)?;
            let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
            Ok(WebhookResponse {
                id: row.get(0)?,
                board_id: row.get(1)?,
                url: row.get(2)?,
                secret: None,
                events,
                active: row.get::<_, i32>(4)? == 1,
                failure_count: row.get(5)?,
                last_triggered_at: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(webhooks))
}

/// Update a webhook — requires manage key.
#[patch("/boards/<board_id>/webhooks/<webhook_id>", format = "json", data = "<req>")]
pub fn update_webhook(
    board_id: &str,
    webhook_id: &str,
    req: Json<UpdateWebhookRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<WebhookResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM webhooks WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![webhook_id, board_id],
        |row| row.get(0),
    ).unwrap_or(false);

    if !exists { return Err(not_found("Webhook")); }

    if let Some(ref url) = req.url {
        if url.trim().is_empty() {
            return Err((Status::BadRequest, Json(ApiError {
                error: "Webhook URL cannot be empty".to_string(),
                code: "EMPTY_URL".to_string(),
                status: 400,
            })));
        }
        conn.execute("UPDATE webhooks SET url = ?1 WHERE id = ?2", rusqlite::params![url.trim(), webhook_id])
            .map_err(|e| db_error(&e.to_string()))?;
    }

    if let Some(ref events) = req.events {
        let valid_events = [
            "task.created", "task.updated", "task.deleted", "task.claimed",
            "task.released", "task.moved", "task.reordered", "task.comment",
            "task.dependency.added", "task.dependency.removed",
        ];
        for ev in events {
            if !valid_events.contains(&ev.as_str()) {
                return Err((Status::BadRequest, Json(ApiError {
                    error: format!("Invalid event type '{}'", ev),
                    code: "INVALID_EVENT_TYPE".to_string(),
                    status: 400,
                })));
            }
        }
        let events_json = serde_json::to_string(events).unwrap_or_else(|_| "[]".to_string());
        conn.execute("UPDATE webhooks SET events = ?1 WHERE id = ?2", rusqlite::params![events_json, webhook_id])
            .map_err(|e| db_error(&e.to_string()))?;
    }

    if let Some(active) = req.active {
        let active_int: i32 = if active { 1 } else { 0 };
        if active {
            conn.execute("UPDATE webhooks SET active = ?1, failure_count = 0 WHERE id = ?2", rusqlite::params![active_int, webhook_id])
                .map_err(|e| db_error(&e.to_string()))?;
        } else {
            conn.execute("UPDATE webhooks SET active = ?1 WHERE id = ?2", rusqlite::params![active_int, webhook_id])
                .map_err(|e| db_error(&e.to_string()))?;
        }
    }

    let wh = conn.query_row(
        "SELECT id, board_id, url, events, active, failure_count, last_triggered_at, created_at FROM webhooks WHERE id = ?1",
        rusqlite::params![webhook_id],
        |row| {
            let events_str: String = row.get(3)?;
            let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
            Ok(WebhookResponse {
                id: row.get(0)?, board_id: row.get(1)?, url: row.get(2)?,
                secret: None, events, active: row.get::<_, i32>(4)? == 1,
                failure_count: row.get(5)?, last_triggered_at: row.get(6)?, created_at: row.get(7)?,
            })
        },
    ).map_err(|_| not_found("Webhook"))?;

    Ok(Json(wh))
}

/// Delete a webhook — requires manage key.
#[delete("/boards/<board_id>/webhooks/<webhook_id>")]
pub fn delete_webhook(
    board_id: &str,
    webhook_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let affected = conn
        .execute("DELETE FROM webhooks WHERE id = ?1 AND board_id = ?2", rusqlite::params![webhook_id, board_id])
        .unwrap_or(0);

    if affected > 0 {
        Ok(Json(serde_json::json!({"deleted": true, "id": webhook_id})))
    } else {
        Err(not_found("Webhook"))
    }
}
