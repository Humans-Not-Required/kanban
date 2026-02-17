use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool, DbPoolExt};
use crate::models::*;

use super::db_error;

/// Create a column — requires manage key.
#[post("/boards/<board_id>/columns", format = "json", data = "<req>")]
pub fn create_column(
    board_id: &str,
    req: Json<CreateColumnRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<ColumnResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.conn();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    let position = req.position.unwrap_or_else(|| {
        conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM columns WHERE board_id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    });

    let col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position, wip_limit) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![col_id, board_id, req.name, position, req.wip_limit],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    Ok(Json(ColumnResponse {
        id: col_id,
        name: req.name,
        position,
        wip_limit: req.wip_limit,
        task_count: 0,
    }))
}

/// Update a column (rename, change WIP limit) — requires manage key.
#[patch("/boards/<board_id>/columns/<column_id>", format = "json", data = "<req>")]
pub fn update_column(
    board_id: &str,
    column_id: &str,
    req: Json<UpdateColumnRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<ColumnResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.conn();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    // Verify column exists and belongs to this board
    let col: (String, i32, Option<i32>) = conn
        .query_row(
            "SELECT name, position, wip_limit FROM columns WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![column_id, board_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| {
            (
                Status::NotFound,
                Json(ApiError {
                    error: "Column not found".to_string(),
                    code: "COLUMN_NOT_FOUND".to_string(),
                    status: 404,
                }),
            )
        })?;

    let new_name = req.name.unwrap_or(col.0);
    let new_wip = match req.wip_limit {
        Some(wip) => wip, // explicitly set (Some(n) or None to clear)
        None => col.2,    // not provided, keep existing
    };

    conn.execute(
        "UPDATE columns SET name = ?1, wip_limit = ?2 WHERE id = ?3 AND board_id = ?4",
        rusqlite::params![new_name, new_wip, column_id, board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let task_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE column_id = ?1",
            rusqlite::params![column_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(Json(ColumnResponse {
        id: column_id.to_string(),
        name: new_name,
        position: col.1,
        wip_limit: new_wip,
        task_count,
    }))
}

/// Delete a column — requires manage key.
/// Fails if the column still contains tasks (must move/delete them first).
#[delete("/boards/<board_id>/columns/<column_id>")]
pub fn delete_column(
    board_id: &str,
    column_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.conn();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    // Verify column exists and belongs to this board
    let col_position: i32 = conn
        .query_row(
            "SELECT position FROM columns WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![column_id, board_id],
            |row| row.get(0),
        )
        .map_err(|_| {
            (
                Status::NotFound,
                Json(ApiError {
                    error: "Column not found".to_string(),
                    code: "COLUMN_NOT_FOUND".to_string(),
                    status: 404,
                }),
            )
        })?;

    // Check if column has tasks
    let task_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE column_id = ?1",
            rusqlite::params![column_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if task_count > 0 {
        return Err((
            Status::Conflict,
            Json(ApiError {
                error: format!(
                    "Column has {} task(s). Move or delete them before removing the column.",
                    task_count
                ),
                code: "COLUMN_NOT_EMPTY".to_string(),
                status: 409,
            }),
        ));
    }

    // Count total columns — prevent deleting the last one
    let total_columns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM columns WHERE board_id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if total_columns <= 1 {
        return Err((
            Status::Conflict,
            Json(ApiError {
                error: "Cannot delete the last column. A board must have at least one column."
                    .to_string(),
                code: "LAST_COLUMN".to_string(),
                status: 409,
            }),
        ));
    }

    // Delete the column
    conn.execute(
        "DELETE FROM columns WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![column_id, board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    // Shift positions of columns after the deleted one
    conn.execute(
        "UPDATE columns SET position = position - 1 WHERE board_id = ?1 AND position > ?2",
        rusqlite::params![board_id, col_position],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    Ok(Json(serde_json::json!({ "deleted": true, "column_id": column_id })))
}

/// Reorder columns — requires manage key.
/// Accepts a list of column IDs in the desired order.
#[post("/boards/<board_id>/columns/reorder", format = "json", data = "<req>")]
pub fn reorder_columns(
    board_id: &str,
    req: Json<ReorderColumnsRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<Vec<ColumnResponse>>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.conn();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    // Get existing column IDs for this board
    let mut stmt = conn
        .prepare("SELECT id FROM columns WHERE board_id = ?1")
        .map_err(|e| db_error(&e.to_string()))?;
    let existing_ids: Vec<String> = stmt
        .query_map(rusqlite::params![board_id], |row| row.get(0))
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    // Validate: must contain exactly the same set of column IDs
    if req.column_ids.len() != existing_ids.len() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: format!(
                    "Expected {} column IDs, got {}",
                    existing_ids.len(),
                    req.column_ids.len()
                ),
                code: "INVALID_COLUMN_LIST".to_string(),
                status: 400,
            }),
        ));
    }

    for cid in &req.column_ids {
        if !existing_ids.contains(cid) {
            return Err((
                Status::BadRequest,
                Json(ApiError {
                    error: format!("Column {} not found in this board", cid),
                    code: "COLUMN_NOT_FOUND".to_string(),
                    status: 400,
                }),
            )
            );
        }
    }

    // Update positions
    for (i, col_id) in req.column_ids.iter().enumerate() {
        conn.execute(
            "UPDATE columns SET position = ?1 WHERE id = ?2 AND board_id = ?3",
            rusqlite::params![i as i32, col_id, board_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    }

    // Return updated columns
    let mut col_stmt = conn
        .prepare(
            "SELECT c.id, c.name, c.position, c.wip_limit,
                    (SELECT COUNT(*) FROM tasks WHERE column_id = c.id) as task_count
             FROM columns c WHERE c.board_id = ?1 ORDER BY c.position",
        )
        .map_err(|e| db_error(&e.to_string()))?;

    let columns: Vec<ColumnResponse> = col_stmt
        .query_map(rusqlite::params![board_id], |row| {
            Ok(ColumnResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                position: row.get(2)?,
                wip_limit: row.get(3)?,
                task_count: row.get(4)?,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(columns))
}
