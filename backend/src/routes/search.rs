use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::access;
use crate::db::DbPool;
use crate::models::*;

use super::{db_error, row_to_task};

/// Search tasks — public, no auth required.
#[allow(clippy::too_many_arguments)]
#[get(
    "/boards/<board_id>/tasks/search?<q>&<column>&<assigned>&<priority>&<label>&<archived>&<limit>&<offset>"
)]
pub fn search_tasks(
    board_id: &str,
    q: &str,
    column: Option<&str>,
    assigned: Option<&str>,
    priority: Option<i32>,
    label: Option<&str>,
    archived: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
    db: &State<DbPool>,
) -> Result<Json<SearchResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_board_exists(&conn, board_id)?;

    let query = q.trim();
    if query.is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Search query cannot be empty".to_string(),
                code: "EMPTY_QUERY".to_string(),
                status: 400,
            }),
        ));
    }

    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0).max(0);
    let like_pattern = format!("%{}%", query);

    let mut sql = String::from(
        "SELECT t.id, t.board_id, t.column_id, c.name, t.title, t.description,
                t.priority, t.position, t.created_by, t.assigned_to, t.claimed_by,
                t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at, t.archived_at,
                t.created_at, t.updated_at,
                (SELECT COUNT(*) FROM task_events te WHERE te.task_id = t.id AND te.event_type = 'comment') as comment_count
         FROM tasks t
         JOIN columns c ON t.column_id = c.id
         WHERE t.board_id = ?1
           AND (t.title LIKE ?2 OR t.description LIKE ?2 OR t.labels LIKE ?2)",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
        Box::new(board_id.to_string()),
        Box::new(like_pattern.clone()),
    ];

    if let Some(col) = column {
        params.push(Box::new(col.to_string()));
        sql.push_str(&format!(" AND t.column_id = ?{}", params.len()));
    }
    if let Some(a) = assigned {
        params.push(Box::new(a.to_string()));
        sql.push_str(&format!(" AND t.assigned_to = ?{}", params.len()));
    }
    if let Some(p) = priority {
        params.push(Box::new(p));
        sql.push_str(&format!(" AND t.priority >= ?{}", params.len()));
    }
    if let Some(l) = label {
        params.push(Box::new(format!("%\"{}\"%", l)));
        sql.push_str(&format!(" AND t.labels LIKE ?{}", params.len()));
    }

    match archived {
        Some(true) => sql.push_str(" AND t.archived_at IS NOT NULL"),
        _ => sql.push_str(" AND t.archived_at IS NULL"),
    }

    let count_sql = sql.replace(
        "SELECT t.id, t.board_id, t.column_id, c.name, t.title, t.description,
                t.priority, t.position, t.created_by, t.assigned_to, t.claimed_by,
                t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at, t.archived_at,
                t.created_at, t.updated_at,
                (SELECT COUNT(*) FROM task_events te WHERE te.task_id = t.id AND te.event_type = 'comment') as comment_count",
        "SELECT COUNT(*)",
    );
    let count_param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let total: i64 = conn.query_row(&count_sql, count_param_refs.as_slice(), |row| row.get(0)).unwrap_or(0);

    sql.push_str(&format!(
        " ORDER BY CASE WHEN t.title LIKE ?{p} THEN 0 ELSE 1 END, t.priority DESC, t.updated_at DESC LIMIT ?{l} OFFSET ?{o}",
        p = params.len() + 1,
        l = params.len() + 2,
        o = params.len() + 3,
    ));
    params.push(Box::new(like_pattern));
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| db_error(&e.to_string()))?;

    let tasks: Vec<TaskResponse> = stmt
        .query_map(param_refs.as_slice(), row_to_task)
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(SearchResponse { query: query.to_string(), tasks, total, limit, offset }))
}
