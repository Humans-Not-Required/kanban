use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool};
use crate::events::EventBus;
use crate::models::*;

use super::{db_error, extract_mentions, next_event_seq};

/// Post a comment on a task — requires manage key.
#[post(
    "/boards/<board_id>/tasks/<task_id>/comment",
    format = "json",
    data = "<body>"
)]
pub fn comment_on_task(
    board_id: &str,
    task_id: &str,
    body: Json<serde_json::Value>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskEventResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let actor = body
        .get("actor_name")
        .and_then(|v| v.as_str())
        .unwrap_or("anonymous")
        .to_string();

    access::require_display_name_if_needed(&conn, board_id, &actor)?;

    let message = body.get("message").and_then(|v| v.as_str()).unwrap_or("");

    if message.is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Comment message cannot be empty".to_string(),
                code: "EMPTY_MESSAGE".to_string(),
                status: 400,
            }),
        ));
    }

    let event_id = uuid::Uuid::new_v4().to_string();
    let mentions = extract_mentions(message);
    let data = if mentions.is_empty() {
        serde_json::json!({"message": message, "actor": actor})
    } else {
        serde_json::json!({"message": message, "actor": actor, "mentions": mentions})
    };
    let data_str = serde_json::to_string(&data).unwrap();
    let seq = next_event_seq(&conn);

    conn.execute(
        "INSERT INTO task_events (id, task_id, event_type, actor, data, seq) VALUES (?1, ?2, 'comment', ?3, ?4, ?5)",
        rusqlite::params![event_id, task_id, actor, data_str, seq],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let created_at: String = conn
        .query_row(
            "SELECT created_at FROM task_events WHERE id = ?1",
            rusqlite::params![event_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());

    bus.emit(crate::events::BoardEvent {
        event: "task.comment".to_string(),
        board_id: board_id.to_string(),
        data: serde_json::json!({"task_id": task_id, "actor": &actor, "message": message, "mentions": &mentions}),
    });

    Ok(Json(TaskEventResponse {
        id: event_id,
        event_type: "comment".to_string(),
        actor,
        data,
        created_at,
    }))
}
