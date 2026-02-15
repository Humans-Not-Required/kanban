use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use rusqlite::Connection;

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool};
use crate::events::EventBus;
use crate::models::*;

use super::{db_error, not_found, log_event};

/// Create a dependency — requires manage key.
#[post("/boards/<board_id>/dependencies", format = "json", data = "<req>")]
pub fn create_dependency(
    board_id: &str,
    req: Json<CreateDependencyRequest>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<DependencyResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    if req.blocker_task_id == req.blocked_task_id {
        return Err((Status::BadRequest, Json(ApiError {
            error: "A task cannot depend on itself".to_string(),
            code: "SELF_DEPENDENCY".to_string(),
            status: 400,
        })));
    }

    let blocker_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![req.blocker_task_id, board_id], |row| row.get(0),
    ).unwrap_or(false);

    let blocked_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![req.blocked_task_id, board_id], |row| row.get(0),
    ).unwrap_or(false);

    if !blocker_exists { return Err(not_found("Blocker task")); }
    if !blocked_exists { return Err(not_found("Blocked task")); }

    let reverse_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM task_dependencies WHERE blocker_task_id = ?1 AND blocked_task_id = ?2",
        rusqlite::params![req.blocked_task_id, req.blocker_task_id], |row| row.get(0),
    ).unwrap_or(false);

    if reverse_exists {
        return Err((Status::Conflict, Json(ApiError {
            error: "Circular dependency: the reverse relationship already exists".to_string(),
            code: "CIRCULAR_DEPENDENCY".to_string(),
            status: 409,
        })));
    }

    if has_path(&conn, &req.blocked_task_id, &req.blocker_task_id) {
        return Err((Status::Conflict, Json(ApiError {
            error: "Circular dependency: this would create a cycle in the dependency graph".to_string(),
            code: "CIRCULAR_DEPENDENCY".to_string(),
            status: 409,
        })));
    }

    let dep_id = uuid::Uuid::new_v4().to_string();
    let result = conn.execute(
        "INSERT INTO task_dependencies (id, board_id, blocker_task_id, blocked_task_id, note) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![dep_id, board_id, req.blocker_task_id, req.blocked_task_id, req.note],
    );

    match result {
        Ok(_) => {}
        Err(e) if e.to_string().contains("UNIQUE") => {
            return Err((Status::Conflict, Json(ApiError {
                error: "This dependency already exists".to_string(),
                code: "DUPLICATE_DEPENDENCY".to_string(),
                status: 409,
            })));
        }
        Err(e) => return Err(db_error(&e.to_string())),
    }

    let event_data = serde_json::json!({
        "dependency_id": dep_id,
        "blocker_task_id": req.blocker_task_id,
        "blocked_task_id": req.blocked_task_id,
        "note": req.note,
    });
    log_event(&conn, &req.blocked_task_id, "dependency.added", "anonymous", &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.dependency.added".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_dependency_response(&conn, &dep_id)
}

/// List dependencies — public, no auth required.
#[get("/boards/<board_id>/dependencies?<task>")]
pub fn list_dependencies(
    board_id: &str,
    task: Option<&str>,
    db: &State<DbPool>,
) -> Result<Json<Vec<DependencyResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_board_exists(&conn, board_id)?;

    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(task_id) = task {
        (
            "SELECT d.id, d.board_id, d.blocker_task_id, bt.title, bc.name, bt.completed_at IS NOT NULL,
                    d.blocked_task_id, blt.title, blc.name, d.note, d.created_by, d.created_at
             FROM task_dependencies d
             JOIN tasks bt ON d.blocker_task_id = bt.id
             JOIN columns bc ON bt.column_id = bc.id
             JOIN tasks blt ON d.blocked_task_id = blt.id
             JOIN columns blc ON blt.column_id = blc.id
             WHERE d.board_id = ?1 AND (d.blocker_task_id = ?2 OR d.blocked_task_id = ?2)
             ORDER BY d.created_at ASC".to_string(),
            vec![Box::new(board_id.to_string()) as Box<dyn rusqlite::types::ToSql>, Box::new(task_id.to_string())],
        )
    } else {
        (
            "SELECT d.id, d.board_id, d.blocker_task_id, bt.title, bc.name, bt.completed_at IS NOT NULL,
                    d.blocked_task_id, blt.title, blc.name, d.note, d.created_by, d.created_at
             FROM task_dependencies d
             JOIN tasks bt ON d.blocker_task_id = bt.id
             JOIN columns bc ON bt.column_id = bc.id
             JOIN tasks blt ON d.blocked_task_id = blt.id
             JOIN columns blc ON blt.column_id = blc.id
             WHERE d.board_id = ?1
             ORDER BY d.created_at ASC".to_string(),
            vec![Box::new(board_id.to_string()) as Box<dyn rusqlite::types::ToSql>],
        )
    };

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| db_error(&e.to_string()))?;

    let deps: Vec<DependencyResponse> = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(DependencyResponse {
                id: row.get(0)?, board_id: row.get(1)?,
                blocker_task_id: row.get(2)?, blocker_title: row.get(3)?,
                blocker_column: row.get(4)?, blocker_completed: row.get(5)?,
                blocked_task_id: row.get(6)?, blocked_title: row.get(7)?,
                blocked_column: row.get(8)?, note: row.get(9)?,
                created_by: row.get(10)?, created_at: row.get(11)?,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(deps))
}

/// Delete a dependency — requires manage key.
#[delete("/boards/<board_id>/dependencies/<dep_id>")]
pub fn delete_dependency(
    board_id: &str,
    dep_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    let dep_info = conn.query_row(
        "SELECT blocker_task_id, blocked_task_id FROM task_dependencies WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![dep_id, board_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    );

    let (blocker_id, blocked_id) = match dep_info {
        Ok(info) => info,
        Err(_) => return Err(not_found("Dependency")),
    };

    let affected = conn
        .execute("DELETE FROM task_dependencies WHERE id = ?1 AND board_id = ?2", rusqlite::params![dep_id, board_id])
        .unwrap_or(0);

    if affected > 0 {
        let event_data = serde_json::json!({
            "dependency_id": dep_id,
            "blocker_task_id": blocker_id,
            "blocked_task_id": blocked_id,
        });
        log_event(&conn, &blocked_id, "dependency.removed", "anonymous", &event_data);
        bus.emit(crate::events::BoardEvent {
            event: "task.dependency.removed".to_string(),
            board_id: board_id.to_string(),
            data: event_data,
        });
        Ok(Json(serde_json::json!({"deleted": true, "id": dep_id})))
    } else {
        Err(not_found("Dependency"))
    }
}

// ============ Internal Helpers ============

fn has_path(conn: &Connection, from_task: &str, to_task: &str) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(from_task.to_string());

    while let Some(current) = queue.pop_front() {
        if current == to_task { return true; }
        if !visited.insert(current.clone()) { continue; }
        if let Ok(mut stmt) = conn.prepare("SELECT blocked_task_id FROM task_dependencies WHERE blocker_task_id = ?1") {
            if let Ok(rows) = stmt.query_map(rusqlite::params![current], |row| row.get::<_, String>(0)) {
                for row in rows.flatten() {
                    if !visited.contains(&row) { queue.push_back(row); }
                }
            }
        }
    }
    false
}

fn load_dependency_response(conn: &Connection, dep_id: &str) -> Result<Json<DependencyResponse>, (Status, Json<ApiError>)> {
    conn.query_row(
        "SELECT d.id, d.board_id, d.blocker_task_id, bt.title, bc.name, bt.completed_at IS NOT NULL,
                d.blocked_task_id, blt.title, blc.name, d.note, d.created_by, d.created_at
         FROM task_dependencies d
         JOIN tasks bt ON d.blocker_task_id = bt.id
         JOIN columns bc ON bt.column_id = bc.id
         JOIN tasks blt ON d.blocked_task_id = blt.id
         JOIN columns blc ON blt.column_id = blc.id
         WHERE d.id = ?1",
        rusqlite::params![dep_id],
        |row| {
            Ok(DependencyResponse {
                id: row.get(0)?, board_id: row.get(1)?,
                blocker_task_id: row.get(2)?, blocker_title: row.get(3)?,
                blocker_column: row.get(4)?, blocker_completed: row.get(5)?,
                blocked_task_id: row.get(6)?, blocked_title: row.get(7)?,
                blocked_column: row.get(8)?, note: row.get(9)?,
                created_by: row.get(10)?, created_at: row.get(11)?,
            })
        },
    )
    .map(Json)
    .map_err(|_| not_found("Dependency"))
}
