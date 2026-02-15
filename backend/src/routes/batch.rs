use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use rusqlite::Connection;

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool};
use crate::events::EventBus;
use crate::models::*;

use super::{normalize_labels, log_event};

/// Batch operations — requires manage key.
#[post("/boards/<board_id>/tasks/batch", format = "json", data = "<req>")]
pub fn batch_tasks(
    board_id: &str,
    req: Json<BatchRequest>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<BatchResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;
    let actor = req.actor_name.as_deref().unwrap_or("batch");
    access::require_display_name_if_needed(&conn, board_id, actor)?;

    if req.operations.is_empty() {
        return Err((Status::BadRequest, Json(ApiError {
            error: "No operations provided".to_string(),
            code: "EMPTY_BATCH".to_string(),
            status: 400,
        })));
    }

    if req.operations.len() > 50 {
        return Err((Status::BadRequest, Json(ApiError {
            error: "Maximum 50 operations per batch request".to_string(),
            code: "BATCH_TOO_LARGE".to_string(),
            status: 400,
        })));
    }

    let mut results = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;

    for op in &req.operations {
        match op {
            BatchOperation::Move { task_ids, column_id } => {
                let result = batch_move(&conn, board_id, task_ids, column_id, actor, bus);
                match result {
                    Ok(affected) => { succeeded += 1; results.push(BatchOperationResult { action: "move".to_string(), task_ids: task_ids.clone(), success: true, error: None, affected }); }
                    Err(msg) => { failed += 1; results.push(BatchOperationResult { action: "move".to_string(), task_ids: task_ids.clone(), success: false, error: Some(msg), affected: 0 }); }
                }
            }
            BatchOperation::Update { task_ids, fields } => {
                let result = batch_update(&conn, board_id, task_ids, fields, actor, bus);
                match result {
                    Ok(affected) => { succeeded += 1; results.push(BatchOperationResult { action: "update".to_string(), task_ids: task_ids.clone(), success: true, error: None, affected }); }
                    Err(msg) => { failed += 1; results.push(BatchOperationResult { action: "update".to_string(), task_ids: task_ids.clone(), success: false, error: Some(msg), affected: 0 }); }
                }
            }
            BatchOperation::Delete { task_ids } => {
                let result = batch_delete(&conn, board_id, task_ids, actor, bus);
                match result {
                    Ok(affected) => { succeeded += 1; results.push(BatchOperationResult { action: "delete".to_string(), task_ids: task_ids.clone(), success: true, error: None, affected }); }
                    Err(msg) => { failed += 1; results.push(BatchOperationResult { action: "delete".to_string(), task_ids: task_ids.clone(), success: false, error: Some(msg), affected: 0 }); }
                }
            }
        }
    }

    Ok(Json(BatchResponse { total: req.operations.len(), succeeded, failed, results }))
}

fn batch_move(conn: &Connection, board_id: &str, task_ids: &[String], column_id: &str, actor: &str, bus: &EventBus) -> Result<usize, String> {
    let col_exists: bool = conn.query_row("SELECT COUNT(*) > 0 FROM columns WHERE id = ?1 AND board_id = ?2", rusqlite::params![column_id, board_id], |row| row.get(0)).unwrap_or(false);
    if !col_exists { return Err("Target column not found in this board".to_string()); }

    let is_done_column: bool = conn.query_row("SELECT position = (SELECT MAX(position) FROM columns WHERE board_id = ?1) FROM columns WHERE id = ?2", rusqlite::params![board_id, column_id], |row| row.get(0)).unwrap_or(false);

    let mut affected = 0;
    for task_id in task_ids {
        let belongs: bool = conn.query_row("SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2", rusqlite::params![task_id, board_id], |row| row.get(0)).unwrap_or(false);
        if !belongs { continue; }

        let from_col: String = conn.query_row("SELECT column_id FROM tasks WHERE id = ?1", rusqlite::params![task_id], |row| row.get(0)).unwrap_or_default();

        let rows = if is_done_column {
            conn.execute("UPDATE tasks SET column_id = ?1, completed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?2 AND board_id = ?3", rusqlite::params![column_id, task_id, board_id]).unwrap_or(0)
        } else {
            conn.execute("UPDATE tasks SET column_id = ?1, completed_at = NULL, updated_at = datetime('now') WHERE id = ?2 AND board_id = ?3", rusqlite::params![column_id, task_id, board_id]).unwrap_or(0)
        };

        if rows > 0 {
            affected += 1;
            let from_col_name: String = conn.query_row("SELECT name FROM columns WHERE id = ?1", rusqlite::params![from_col], |row| row.get(0)).unwrap_or_else(|_| from_col.clone());
            let to_col_name: String = conn.query_row("SELECT name FROM columns WHERE id = ?1", rusqlite::params![column_id], |row| row.get(0)).unwrap_or_else(|_| column_id.to_string());
            let event_data = serde_json::json!({"task_id": task_id, "from": from_col, "to": column_id, "from_column": from_col_name, "to_column": to_col_name, "batch": true});
            log_event(conn, task_id, "moved", actor, &event_data);
            bus.emit(crate::events::BoardEvent { event: "task.moved".to_string(), board_id: board_id.to_string(), data: event_data });
        }
    }
    Ok(affected)
}

fn batch_update(conn: &Connection, board_id: &str, task_ids: &[String], fields: &BatchUpdateFields, actor: &str, bus: &EventBus) -> Result<usize, String> {
    let mut affected = 0;
    for task_id in task_ids {
        let belongs: bool = conn.query_row("SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2", rusqlite::params![task_id, board_id], |row| row.get(0)).unwrap_or(false);
        if !belongs { continue; }

        let mut changes = serde_json::Map::new();

        if let Some(p) = fields.priority {
            conn.execute("UPDATE tasks SET priority = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![p, task_id]).ok();
            changes.insert("priority".into(), serde_json::json!(p));
        }
        if let Some(ref assigned) = fields.assigned_to {
            conn.execute("UPDATE tasks SET assigned_to = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![assigned, task_id]).ok();
            changes.insert("assigned_to".into(), serde_json::json!(assigned));
        }
        if let Some(ref labels) = fields.labels {
            let normalized = normalize_labels(labels);
            let labels_json = serde_json::to_string(&normalized).unwrap_or_else(|_| "[]".to_string());
            conn.execute("UPDATE tasks SET labels = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![labels_json, task_id]).ok();
            changes.insert("labels".into(), serde_json::json!(normalized));
        }
        if let Some(ref due) = fields.due_at {
            conn.execute("UPDATE tasks SET due_at = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![due, task_id]).ok();
            changes.insert("due_at".into(), serde_json::json!(due));
        }

        if !changes.is_empty() {
            affected += 1;
            let event_data = serde_json::Value::Object(changes.clone());
            log_event(conn, task_id, "updated", actor, &event_data);
            let mut emit_data = changes;
            emit_data.insert("task_id".into(), serde_json::json!(task_id));
            emit_data.insert("batch".into(), serde_json::json!(true));
            bus.emit(crate::events::BoardEvent { event: "task.updated".to_string(), board_id: board_id.to_string(), data: serde_json::Value::Object(emit_data) });
        }
    }
    Ok(affected)
}

fn batch_delete(conn: &Connection, board_id: &str, task_ids: &[String], actor: &str, bus: &EventBus) -> Result<usize, String> {
    let mut affected = 0;
    for task_id in task_ids {
        let task_title: Option<String> = conn.query_row("SELECT title FROM tasks WHERE id = ?1 AND board_id = ?2", rusqlite::params![task_id, board_id], |row| row.get(0)).ok();
        let rows = conn.execute("DELETE FROM tasks WHERE id = ?1 AND board_id = ?2", rusqlite::params![task_id, board_id]).unwrap_or(0);
        if rows > 0 {
            affected += 1;
            let event_data = serde_json::json!({"task_id": task_id, "title": task_title, "batch": true});
            log_event(conn, task_id, "deleted", actor, &event_data);
            bus.emit(crate::events::BoardEvent { event: "task.deleted".to_string(), board_id: board_id.to_string(), data: event_data });
        }
    }
    Ok(affected)
}
