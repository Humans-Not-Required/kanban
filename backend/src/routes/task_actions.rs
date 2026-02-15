use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool};
use crate::events::EventBus;
use crate::models::*;

use super::{db_error, not_found, check_wip_limit, log_event, load_task_response};

/// Claim a task — requires manage key.
#[post("/boards/<board_id>/tasks/<task_id>/claim?<actor>")]
pub fn claim_task(
    board_id: &str,
    task_id: &str,
    actor: Option<&str>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    let actor = actor.unwrap_or("anonymous").to_string();
    access::require_display_name_if_needed(&conn, board_id, &actor)?;

    let current_claim: Option<String> = conn
        .query_row(
            "SELECT claimed_by FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![task_id, board_id],
            |row| row.get(0),
        )
        .map_err(|_| not_found("Task"))?;

    if let Some(ref claimer) = current_claim {
        if claimer != &actor {
            return Err((
                Status::Conflict,
                Json(ApiError {
                    error: format!("Task already claimed by '{}'", claimer),
                    code: "ALREADY_CLAIMED".to_string(),
                    status: 409,
                }),
            ));
        }
    }

    conn.execute(
        "UPDATE tasks SET claimed_by = ?1, claimed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?2 AND board_id = ?3",
        rusqlite::params![actor, task_id, board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let event_data = serde_json::json!({"task_id": task_id, "actor": actor});
    log_event(&conn, task_id, "claimed", &actor, &event_data);
    bus.emit(crate::events::BoardEvent {
        event: "task.claimed".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

/// Release a claimed task — requires manage key.
#[post("/boards/<board_id>/tasks/<task_id>/release?<actor>")]
pub fn release_task(
    board_id: &str,
    task_id: &str,
    actor: Option<&str>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let actor = actor.unwrap_or("anonymous");
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;
    access::require_display_name_if_needed(&conn, board_id, actor)?;

    conn.execute(
        "UPDATE tasks SET claimed_by = NULL, claimed_at = NULL, updated_at = datetime('now') WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![task_id, board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let event_data = serde_json::json!({"task_id": task_id});
    log_event(&conn, task_id, "released", actor, &event_data);
    bus.emit(crate::events::BoardEvent {
        event: "task.released".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

/// Move a task to a different column — requires manage key.
#[post("/boards/<board_id>/tasks/<task_id>/move/<target_column_id>?<actor>")]
pub fn move_task(
    board_id: &str,
    task_id: &str,
    target_column_id: &str,
    actor: Option<&str>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let actor = actor.unwrap_or("anonymous");
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;
    access::require_display_name_if_needed(&conn, board_id, actor)?;

    let col_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM columns WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![target_column_id, board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !col_exists {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Target column not found in this board".to_string(),
                code: "INVALID_COLUMN".to_string(),
                status: 400,
            }),
        ));
    }

    check_wip_limit(&conn, target_column_id, Some(task_id))?;

    let from_col: String = conn
        .query_row(
            "SELECT column_id FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![task_id, board_id],
            |row| row.get(0),
        )
        .map_err(|_| not_found("Task"))?;

    let is_done_column: bool = conn
        .query_row(
            "SELECT position = (SELECT MAX(position) FROM columns WHERE board_id = ?1) FROM columns WHERE id = ?2",
            rusqlite::params![board_id, target_column_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if is_done_column {
        conn.execute(
            "UPDATE tasks SET column_id = ?1, completed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?2 AND board_id = ?3",
            rusqlite::params![target_column_id, task_id, board_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    } else {
        conn.execute(
            "UPDATE tasks SET column_id = ?1, completed_at = NULL, updated_at = datetime('now') WHERE id = ?2 AND board_id = ?3",
            rusqlite::params![target_column_id, task_id, board_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    }

    let from_col_name: String = conn
        .query_row("SELECT name FROM columns WHERE id = ?1", rusqlite::params![from_col], |row| row.get(0))
        .unwrap_or_else(|_| from_col.clone());
    let to_col_name: String = conn
        .query_row("SELECT name FROM columns WHERE id = ?1", rusqlite::params![target_column_id], |row| row.get(0))
        .unwrap_or_else(|_| target_column_id.to_string());

    let event_data = serde_json::json!({"task_id": task_id, "from": from_col, "to": target_column_id, "from_column": from_col_name, "to_column": to_col_name});
    log_event(&conn, task_id, "moved", actor, &event_data);
    bus.emit(crate::events::BoardEvent {
        event: "task.moved".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

/// Reorder a task — requires manage key.
#[post(
    "/boards/<board_id>/tasks/<task_id>/reorder?<actor>",
    format = "json",
    data = "<req>"
)]
pub fn reorder_task(
    board_id: &str,
    task_id: &str,
    actor: Option<&str>,
    req: Json<ReorderTaskRequest>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;
    let actor = actor.unwrap_or("anonymous");
    access::require_display_name_if_needed(&conn, board_id, actor)?;

    let current_column: String = conn
        .query_row(
            "SELECT column_id FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![task_id, board_id],
            |row| row.get(0),
        )
        .map_err(|_| not_found("Task"))?;

    let target_column = req.column_id.as_deref().unwrap_or(&current_column);
    let moving_columns = target_column != current_column;

    if moving_columns {
        let col_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM columns WHERE id = ?1 AND board_id = ?2",
                rusqlite::params![target_column, board_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !col_exists {
            return Err((
                Status::BadRequest,
                Json(ApiError {
                    error: "Target column not found in this board".to_string(),
                    code: "INVALID_COLUMN".to_string(),
                    status: 400,
                }),
            ));
        }

        check_wip_limit(&conn, target_column, Some(task_id))?;
    }

    let new_pos = req.position.max(0);

    if !moving_columns {
        conn.execute(
            "UPDATE tasks SET position = position - 1 WHERE column_id = ?1 AND position > (SELECT position FROM tasks WHERE id = ?2) AND id != ?2",
            rusqlite::params![target_column, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    }

    conn.execute(
        "UPDATE tasks SET position = position + 1 WHERE column_id = ?1 AND position >= ?2 AND id != ?3",
        rusqlite::params![target_column, new_pos, task_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    if moving_columns {
        let is_done_column: bool = conn
            .query_row(
                "SELECT position = (SELECT MAX(position) FROM columns WHERE board_id = ?1) FROM columns WHERE id = ?2",
                rusqlite::params![board_id, target_column],
                |row| row.get(0),
            )
            .unwrap_or(false);

        let completed = if is_done_column { "datetime('now')" } else { "NULL" };

        conn.execute(
            &format!(
                "UPDATE tasks SET column_id = ?1, position = ?2, completed_at = {}, updated_at = datetime('now') WHERE id = ?3",
                completed
            ),
            rusqlite::params![target_column, new_pos, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;

        conn.execute(
            "UPDATE tasks SET position = position - 1 WHERE column_id = ?1 AND position > 0 AND id NOT IN (SELECT id FROM tasks WHERE column_id = ?1 AND position = 0) ORDER BY position",
            rusqlite::params![current_column],
        )
        .ok();
    } else {
        conn.execute(
            "UPDATE tasks SET position = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![new_pos, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    }

    let event_data = serde_json::json!({
        "task_id": task_id,
        "position": new_pos,
        "column_id": target_column,
        "from_column": current_column,
    });
    log_event(&conn, task_id, "reordered", actor, &event_data);
    bus.emit(crate::events::BoardEvent {
        event: "task.reordered".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}
