use chrono::Utc;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool, DbPoolExt};
use crate::events::EventBus;
use crate::models::*;

use super::{db_error, not_found, check_wip_limit, normalize_labels, log_event, load_task_response, row_to_task};

/// Create a task — requires manage key.
#[post("/boards/<board_id>/tasks", format = "json", data = "<req>")]
pub fn create_task(
    board_id: &str,
    req: Json<CreateTaskRequest>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.conn();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    let creator_name = if req.actor_name.is_empty() { "anonymous" } else { &req.actor_name };
    access::require_display_name_if_needed(&conn, board_id, creator_name)?;

    if req.title.trim().is_empty() && req.description.trim().is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Either title or description must be provided".to_string(),
                code: "EMPTY_TASK".to_string(),
                status: 400,
            }),
        ));
    }

    let column_id = match req.column_id {
        Some(ref cid) => {
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM columns WHERE id = ?1 AND board_id = ?2",
                    rusqlite::params![cid, board_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if !exists {
                return Err((
                    Status::BadRequest,
                    Json(ApiError {
                        error: "Column not found in this board".to_string(),
                        code: "INVALID_COLUMN".to_string(),
                        status: 400,
                    }),
                ));
            }
            cid.clone()
        }
        None => conn
            .query_row(
                "SELECT id FROM columns WHERE board_id = ?1 ORDER BY position ASC LIMIT 1",
                rusqlite::params![board_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|_| {
                (
                    Status::BadRequest,
                    Json(ApiError {
                        error: "Board has no columns".to_string(),
                        code: "NO_COLUMNS".to_string(),
                        status: 400,
                    }),
                )
            })?,
    };

    check_wip_limit(&conn, &column_id, None)?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let creator = if req.actor_name.is_empty() {
        "anonymous".to_string()
    } else {
        req.actor_name.clone()
    };
    let normalized_labels = normalize_labels(&req.labels);
    let labels_json = serde_json::to_string(&normalized_labels).unwrap_or_else(|_| "[]".to_string());
    let metadata_json = serde_json::to_string(&req.metadata).unwrap_or_else(|_| "{}".to_string());

    let position: i32 = if let Some(pos) = req.position {
        let pos = pos.max(0);
        conn.execute(
            "UPDATE tasks SET position = position + 1 WHERE column_id = ?1 AND position >= ?2",
            rusqlite::params![column_id, pos],
        )
        .map_err(|e| db_error(&e.to_string()))?;
        pos
    } else {
        conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE column_id = ?1",
            rusqlite::params![column_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    };

    conn.execute(
        "INSERT INTO tasks (id, board_id, column_id, title, description, priority, position, created_by, assigned_to, labels, metadata, due_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            task_id, board_id, column_id, req.title.trim(), req.description,
            req.priority, position, creator, req.assigned_to, labels_json, metadata_json, req.due_at,
        ],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let event_data = serde_json::json!({"title": req.title, "task_id": task_id, "column_id": column_id, "creator": creator});
    log_event(&conn, &task_id, "created", &creator, &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.created".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, &task_id)
}

/// List tasks — public, no auth required.
#[allow(clippy::too_many_arguments)]
#[get("/boards/<board_id>/tasks?<column>&<assigned>&<claimed>&<priority>&<label>&<archived>&<updated_before>&<stale>&<limit>&<offset>")]
pub fn list_tasks(
    board_id: &str,
    column: Option<&str>,
    assigned: Option<&str>,
    claimed: Option<&str>,
    priority: Option<i32>,
    label: Option<&str>,
    archived: Option<bool>,
    updated_before: Option<&str>,
    stale: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
    db: &State<DbPool>,
) -> Result<Json<Vec<TaskResponse>>, (Status, Json<ApiError>)> {
    let conn = db.conn();
    access::require_board_exists(&conn, board_id)?;

    let mut sql = String::from(
        "SELECT t.id, t.board_id, t.column_id, c.name, t.title, t.description,
                t.priority, t.position, t.created_by, t.assigned_to, t.claimed_by,
                t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at, t.archived_at,
                t.created_at, t.updated_at,
                (SELECT COUNT(*) FROM task_events te WHERE te.task_id = t.id AND te.event_type = 'comment') as comment_count
         FROM tasks t
         JOIN columns c ON t.column_id = c.id
         WHERE t.board_id = ?1",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(board_id.to_string())];

    if let Some(col) = column {
        params.push(Box::new(col.to_string()));
        sql.push_str(&format!(" AND t.column_id = ?{}", params.len()));
    }
    if let Some(a) = assigned {
        params.push(Box::new(a.to_string()));
        sql.push_str(&format!(" AND t.assigned_to = ?{}", params.len()));
    }
    if let Some(c) = claimed {
        params.push(Box::new(c.to_string()));
        sql.push_str(&format!(" AND t.claimed_by = ?{}", params.len()));
    }
    if let Some(p) = priority {
        params.push(Box::new(p));
        sql.push_str(&format!(" AND t.priority >= ?{}", params.len()));
    }
    if let Some(l) = label {
        params.push(Box::new(format!("%\"{}\"%", l)));
        sql.push_str(&format!(" AND t.labels LIKE ?{}", params.len()));
    }

    let computed_updated_before = if let Some(minutes) = stale {
        if minutes <= 0 {
            return Err((
                Status::BadRequest,
                Json(ApiError {
                    error: "stale must be a positive number of minutes".into(),
                    code: "INVALID_STALE".into(),
                    status: 400,
                }),
            ));
        }
        Some(
            Utc::now()
                .checked_sub_signed(chrono::Duration::minutes(minutes))
                .unwrap()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
    } else {
        updated_before.map(|s| s.to_string())
    };

    if let Some(ref ub) = computed_updated_before {
        params.push(Box::new(ub.clone()));
        sql.push_str(&format!(" AND t.updated_at < ?{}", params.len()));
    }

    match archived {
        Some(true) => sql.push_str(" AND t.archived_at IS NOT NULL"),
        _ => sql.push_str(" AND t.archived_at IS NULL"),
    }

    sql.push_str(" ORDER BY c.position ASC, t.priority DESC, t.position ASC");

    let effective_limit = limit.unwrap_or(200).clamp(1, 1000);
    let effective_offset = offset.unwrap_or(0).max(0);
    params.push(Box::new(effective_limit));
    sql.push_str(&format!(" LIMIT ?{}", params.len()));
    params.push(Box::new(effective_offset));
    sql.push_str(&format!(" OFFSET ?{}", params.len()));

    let mut stmt = conn.prepare(&sql).map_err(|e| db_error(&e.to_string()))?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let tasks = stmt
        .query_map(param_refs.as_slice(), row_to_task)
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(tasks))
}

/// Get a single task — public, no auth required.
#[get("/boards/<board_id>/tasks/<task_id>")]
pub fn get_task(
    board_id: &str,
    task_id: &str,
    db: &State<DbPool>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.conn();
    access::require_board_exists(&conn, board_id)?;
    load_task_response(&conn, task_id)
}

/// Update a task — requires manage key.
#[patch("/boards/<board_id>/tasks/<task_id>", format = "json", data = "<req>")]
pub fn update_task(
    board_id: &str,
    task_id: &str,
    req: Json<UpdateTaskRequest>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.conn();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;
    let existing = load_task_response(&conn, task_id)?;
    let actor = req.actor_name.clone().unwrap_or_else(|| "anonymous".to_string());
    access::require_display_name_if_needed(&conn, board_id, &actor)?;

    let new_title = req.title.as_deref().unwrap_or(&existing.title);
    let new_desc = req.description.as_deref().unwrap_or(&existing.description);
    if new_title.trim().is_empty() && new_desc.trim().is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Either title or description must be provided".to_string(),
                code: "EMPTY_TASK".to_string(),
                status: 400,
            }),
        ));
    }

    let mut changes = serde_json::Map::new();

    if let Some(ref title) = req.title {
        conn.execute("UPDATE tasks SET title = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![title, task_id]).map_err(|e| db_error(&e.to_string()))?;
        changes.insert("title".into(), serde_json::json!(title));
    }
    if let Some(ref desc) = req.description {
        conn.execute("UPDATE tasks SET description = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![desc, task_id]).map_err(|e| db_error(&e.to_string()))?;
        changes.insert("description".into(), serde_json::json!(desc));
    }
    if let Some(ref col_id) = req.column_id {
        check_wip_limit(&conn, col_id, Some(task_id))?;
        conn.execute("UPDATE tasks SET column_id = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![col_id, task_id]).map_err(|e| db_error(&e.to_string()))?;
        changes.insert("column_id".into(), serde_json::json!(col_id));
    }
    if let Some(p) = req.priority {
        conn.execute("UPDATE tasks SET priority = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![p, task_id]).map_err(|e| db_error(&e.to_string()))?;
        changes.insert("priority".into(), serde_json::json!(p));
    }
    if let Some(ref assigned) = req.assigned_to {
        conn.execute("UPDATE tasks SET assigned_to = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![assigned, task_id]).map_err(|e| db_error(&e.to_string()))?;
        changes.insert("assigned_to".into(), serde_json::json!(assigned));
    }
    if let Some(ref labels) = req.labels {
        let normalized = normalize_labels(labels);
        let labels_json = serde_json::to_string(&normalized).unwrap_or_else(|_| "[]".to_string());
        conn.execute("UPDATE tasks SET labels = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![labels_json, task_id]).map_err(|e| db_error(&e.to_string()))?;
        changes.insert("labels".into(), serde_json::json!(normalized));
    }
    if let Some(ref meta) = req.metadata {
        let meta_json = serde_json::to_string(meta).unwrap_or_else(|_| "{}".to_string());
        conn.execute("UPDATE tasks SET metadata = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![meta_json, task_id]).map_err(|e| db_error(&e.to_string()))?;
        changes.insert("metadata".into(), meta.clone());
    }
    if let Some(ref due) = req.due_at {
        conn.execute("UPDATE tasks SET due_at = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![due, task_id]).map_err(|e| db_error(&e.to_string()))?;
        changes.insert("due_at".into(), serde_json::json!(due));
    }

    if !changes.is_empty() {
        let event_data = serde_json::Value::Object(changes.clone());
        log_event(&conn, task_id, "updated", &actor, &event_data);

        let mut emit_data = changes;
        emit_data.insert("task_id".into(), serde_json::json!(task_id));
        emit_data.insert("actor".into(), serde_json::json!(actor));
        bus.emit(crate::events::BoardEvent {
            event: "task.updated".to_string(),
            board_id: board_id.to_string(),
            data: serde_json::Value::Object(emit_data),
        });
    }

    load_task_response(&conn, task_id)
}

/// Delete a task — requires manage key.
#[delete("/boards/<board_id>/tasks/<task_id>?<actor>")]
pub fn delete_task(
    board_id: &str,
    task_id: &str,
    actor: Option<&str>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.conn();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    let actor = actor.unwrap_or("anonymous");
    access::require_display_name_if_needed(&conn, board_id, actor)?;

    let task_title: Option<String> = conn
        .query_row("SELECT title FROM tasks WHERE id = ?1 AND board_id = ?2", rusqlite::params![task_id, board_id], |row| row.get(0))
        .ok();

    let affected = conn
        .execute("DELETE FROM tasks WHERE id = ?1 AND board_id = ?2", rusqlite::params![task_id, board_id])
        .unwrap_or(0);
    if affected > 0 {
        let event_data = serde_json::json!({"task_id": task_id, "title": task_title});
        log_event(&conn, task_id, "deleted", actor, &event_data);
        bus.emit(crate::events::BoardEvent {
            event: "task.deleted".to_string(),
            board_id: board_id.to_string(),
            data: event_data,
        });
        Ok(Json(serde_json::json!({"deleted": true, "id": task_id})))
    } else {
        Err(not_found("Task"))
    }
}

/// Archive a task — requires manage key.
#[post("/boards/<board_id>/tasks/<task_id>/archive?<actor>")]
pub fn archive_task(
    board_id: &str,
    task_id: &str,
    actor: Option<&str>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let actor = actor.unwrap_or("anonymous");
    let conn = db.conn();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;
    access::require_display_name_if_needed(&conn, board_id, actor)?;

    let _existing = load_task_response(&conn, task_id)?;

    conn.execute(
        "UPDATE tasks SET archived_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![task_id, board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let event_data = serde_json::json!({"task_id": task_id});
    log_event(&conn, task_id, "archived", actor, &event_data);
    bus.emit(crate::events::BoardEvent {
        event: "task.archived".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

/// Unarchive a task — requires manage key.
#[post("/boards/<board_id>/tasks/<task_id>/unarchive?<actor>")]
pub fn unarchive_task(
    board_id: &str,
    task_id: &str,
    actor: Option<&str>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let actor = actor.unwrap_or("anonymous");
    let conn = db.conn();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;
    access::require_display_name_if_needed(&conn, board_id, actor)?;

    let _existing = load_task_response(&conn, task_id)?;

    conn.execute(
        "UPDATE tasks SET archived_at = NULL, updated_at = datetime('now') WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![task_id, board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let event_data = serde_json::json!({"task_id": task_id});
    log_event(&conn, task_id, "unarchived", actor, &event_data);
    bus.emit(crate::events::BoardEvent {
        event: "task.unarchived".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}
