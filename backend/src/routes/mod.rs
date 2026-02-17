// Route modules — decomposed from monolithic routes.rs (3266 lines)
// Each module handles a specific domain; shared helpers live here in mod.rs.

use rocket::http::Status;
use rocket::serde::json::Json;
use rusqlite::Connection;

use crate::models::*;

mod activity;
mod batch;
mod boards;
mod columns;
mod comments;
mod dependencies;
mod search;
mod stream;
mod system;
mod task_actions;
mod tasks;
mod webhook_routes;

// Re-export all public route functions so `kanban::routes::*` still works.
pub use activity::{get_board_activity, get_task_events};
pub use batch::batch_tasks;
pub use boards::{create_board, list_boards, get_board, update_board, archive_board, unarchive_board};
pub use columns::{create_column, update_column, delete_column, reorder_columns};
pub use comments::comment_on_task;
pub use dependencies::{create_dependency, list_dependencies, delete_dependency};
pub use search::search_tasks;
pub use stream::board_event_stream;
pub use system::{health, openapi, llms_txt, root_llms_txt, skills_index, skills_skill_md, spa_fallback};
pub use task_actions::{claim_task, release_task, move_task, reorder_task};
pub use tasks::{create_task, list_tasks, get_task, update_task, delete_task, archive_task, unarchive_task};
pub use webhook_routes::{create_webhook, list_webhooks, update_webhook, delete_webhook};

// ============ Label Normalization ============

/// Normalize a label: lowercase, trim, collapse whitespace → single dash, strip leading/trailing dashes.
pub(crate) fn normalize_label(label: &str) -> String {
    let s: String = label.trim().to_lowercase()
        .split_whitespace().collect::<Vec<_>>().join("-");
    // Collapse multiple dashes, strip leading/trailing dashes
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    s
}

pub(crate) fn normalize_labels(labels: &[String]) -> Vec<String> {
    labels.iter()
        .map(|l| normalize_label(l))
        .filter(|l| !l.is_empty())
        .collect()
}

// ============ @Mention Extraction ============

/// Extract @mentions from text. Supports `@Name` and `@"Name With Spaces"`.
/// Returns deduplicated list of mentioned names (case-preserved).
pub(crate) fn extract_mentions(text: &str) -> Vec<String> {
    let mut mentions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '@' && i + 1 < chars.len() {
            i += 1;
            let name = if chars[i] == '"' {
                // Quoted: @"Name With Spaces"
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '"' {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                if i < chars.len() { i += 1; } // skip closing quote
                name
            } else {
                // Unquoted: @Name (word chars, dots, hyphens)
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-' || chars[i] == '.') {
                    i += 1;
                }
                chars[start..i].iter().collect()
            };
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() {
                let key = trimmed.to_lowercase();
                if seen.insert(key) {
                    mentions.push(trimmed);
                }
            }
        } else {
            i += 1;
        }
    }
    mentions
}

// ============ Shared Helpers ============

pub(crate) fn db_error(msg: &str) -> (Status, Json<ApiError>) {
    (
        Status::InternalServerError,
        Json(ApiError {
            error: format!("Database error: {}", msg),
            code: "DB_ERROR".to_string(),
            status: 500,
        }),
    )
}

pub(crate) fn not_found(entity: &str) -> (Status, Json<ApiError>) {
    (
        Status::NotFound,
        Json(ApiError {
            error: format!("{} not found", entity),
            code: "NOT_FOUND".to_string(),
            status: 404,
        }),
    )
}

/// Check if adding a task to a column would exceed its WIP limit.
pub(crate) fn check_wip_limit(
    conn: &Connection,
    column_id: &str,
    exclude_task_id: Option<&str>,
) -> Result<(), (Status, Json<ApiError>)> {
    let wip_limit: Option<i32> = conn
        .query_row(
            "SELECT wip_limit FROM columns WHERE id = ?1",
            rusqlite::params![column_id],
            |row| row.get(0),
        )
        .map_err(|_| not_found("Column"))?;

    if let Some(limit) = wip_limit {
        let current_count: i32 = match exclude_task_id {
            Some(tid) => conn
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE column_id = ?1 AND id != ?2",
                    rusqlite::params![column_id, tid],
                    |row| row.get(0),
                )
                .unwrap_or(0),
            None => conn
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE column_id = ?1",
                    rusqlite::params![column_id],
                    |row| row.get(0),
                )
                .unwrap_or(0),
        };

        if current_count >= limit {
            let col_name: String = conn
                .query_row(
                    "SELECT name FROM columns WHERE id = ?1",
                    rusqlite::params![column_id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| "unknown".to_string());

            return Err((
                Status::Conflict,
                Json(ApiError {
                    error: format!(
                        "Column '{}' has reached its WIP limit of {} tasks",
                        col_name, limit
                    ),
                    code: "WIP_LIMIT_EXCEEDED".to_string(),
                    status: 409,
                }),
            ));
        }
    }

    Ok(())
}

/// Compute the next monotonic seq value for task_events.
pub(crate) fn next_event_seq(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM task_events",
        [],
        |row| row.get(0),
    )
    .unwrap_or(1)
}

pub(crate) fn log_event(
    conn: &Connection,
    task_id: &str,
    event_type: &str,
    actor: &str,
    data: &serde_json::Value,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let data_str = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    let seq = next_event_seq(conn);
    let _ = conn.execute(
        "INSERT INTO task_events (id, task_id, event_type, actor, data, seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, task_id, event_type, actor, data_str, seq],
    );
}

pub(crate) fn load_board_response(
    conn: &Connection,
    board_id: &str,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let board = conn
        .query_row(
            "SELECT b.id, b.name, b.description, b.archived, b.is_public, b.created_at, b.updated_at,
                    b.quick_done_column_id, b.quick_done_auto_archive,
                    b.quick_reassign_column_id, b.quick_reassign_to,
                    b.require_display_name
             FROM boards b
             WHERE b.id = ?1",
            rusqlite::params![board_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)? == 1,
                    row.get::<_, i32>(4)? == 1,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i32>(8).unwrap_or(0) == 1,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, i32>(11).unwrap_or(0) == 1,
                ))
            },
        )
        .map_err(|_| not_found("Board"))?;

    let mut col_stmt = conn
        .prepare(
            "SELECT c.id, c.name, c.position, c.wip_limit,
                    (SELECT COUNT(*) FROM tasks t WHERE t.column_id = c.id)
             FROM columns c WHERE c.board_id = ?1
             ORDER BY c.position ASC",
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

    let task_count: usize = columns.iter().map(|c| c.task_count as usize).sum();

    Ok(Json(BoardResponse {
        id: board.0,
        name: board.1,
        description: board.2,
        columns,
        task_count,
        archived: board.3,
        is_public: board.4,
        require_display_name: board.11,
        quick_done_column_id: board.7,
        quick_done_auto_archive: board.8,
        quick_reassign_column_id: board.9,
        quick_reassign_to: board.10,
        created_at: board.5,
        updated_at: board.6,
    }))
}

pub(crate) fn load_task_response(
    conn: &Connection,
    task_id: &str,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    conn.query_row(
        "SELECT t.id, t.board_id, t.column_id, c.name, t.title, t.description,
                t.priority, t.position, t.created_by, t.assigned_to, t.claimed_by,
                t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at, t.archived_at,
                t.created_at, t.updated_at,
                (SELECT COUNT(*) FROM task_events te WHERE te.task_id = t.id AND te.event_type = 'comment') as comment_count
         FROM tasks t
         JOIN columns c ON t.column_id = c.id
         WHERE t.id = ?1",
        rusqlite::params![task_id],
        row_to_task,
    )
    .map(Json)
    .map_err(|_| not_found("Task"))
}

pub(crate) fn row_to_task(row: &rusqlite::Row) -> Result<TaskResponse, rusqlite::Error> {
    let labels_str: String = row.get(12)?;
    let meta_str: String = row.get(13)?;

    Ok(TaskResponse {
        id: row.get(0)?,
        board_id: row.get(1)?,
        column_id: row.get(2)?,
        column_name: row.get(3)?,
        title: row.get(4)?,
        description: row.get(5)?,
        priority: row.get(6)?,
        position: row.get(7)?,
        created_by: row.get(8)?,
        assigned_to: row.get(9)?,
        claimed_by: row.get(10)?,
        claimed_at: row.get(11)?,
        labels: serde_json::from_str(&labels_str).unwrap_or_default(),
        metadata: serde_json::from_str(&meta_str).unwrap_or(serde_json::json!({})),
        due_at: row.get(14)?,
        completed_at: row.get(15)?,
        archived_at: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        comment_count: row.get(19).unwrap_or(0),
    })
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_label() {
        assert_eq!(normalize_label("Bug Fix"), "bug-fix");
        assert_eq!(normalize_label("  Feature Request  "), "feature-request");
        assert_eq!(normalize_label("URGENT"), "urgent");
        assert_eq!(normalize_label("multi   space"), "multi-space");
        assert_eq!(normalize_label("already-dashed"), "already-dashed");
        assert_eq!(normalize_label("  "), "");
        assert_eq!(normalize_label("--leading--trailing--"), "leading-trailing");
        assert_eq!(normalize_label("Mixed Case With Spaces"), "mixed-case-with-spaces");
    }

    #[test]
    fn test_normalize_labels() {
        let input = vec!["Bug Fix".to_string(), "  ".to_string(), "FEATURE".to_string()];
        let result = normalize_labels(&input);
        assert_eq!(result, vec!["bug-fix", "feature"]);
    }
}
