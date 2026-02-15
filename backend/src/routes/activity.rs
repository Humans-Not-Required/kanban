use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::access;
use crate::db::DbPool;
use crate::models::*;

use super::{db_error, row_to_task};

/// Get board-level activity feed — all events across all tasks, public, no auth required.
#[get("/boards/<board_id>/activity?<since>&<after>&<limit>&<mentioned>")]
pub fn get_board_activity(
    board_id: &str,
    since: Option<&str>,
    after: Option<i64>,
    limit: Option<u32>,
    mentioned: Option<&str>,
    db: &State<DbPool>,
) -> Result<Json<Vec<BoardActivityItem>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_board_exists(&conn, board_id)?;

    let limit = limit.unwrap_or(50).min(200);

    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(after_seq) = after {
        (
            "SELECT te.id, te.task_id, COALESCE(t.title, '(deleted)'), te.event_type, te.actor, te.data, te.created_at, COALESCE(te.seq, 0)
             FROM task_events te
             LEFT JOIN tasks t ON t.id = te.task_id
             WHERE t.board_id = ?1 AND te.seq > ?2
             ORDER BY te.seq ASC
             LIMIT ?3".to_string(),
            vec![Box::new(board_id.to_string()), Box::new(after_seq), Box::new(limit)],
        )
    } else if let Some(since_ts) = since {
        (
            "SELECT te.id, te.task_id, COALESCE(t.title, '(deleted)'), te.event_type, te.actor, te.data, te.created_at, COALESCE(te.seq, 0)
             FROM task_events te
             LEFT JOIN tasks t ON t.id = te.task_id
             WHERE t.board_id = ?1 AND te.created_at > ?2
             ORDER BY te.created_at DESC
             LIMIT ?3".to_string(),
            vec![Box::new(board_id.to_string()), Box::new(since_ts.to_string()), Box::new(limit)],
        )
    } else {
        (
            "SELECT te.id, te.task_id, COALESCE(t.title, '(deleted)'), te.event_type, te.actor, te.data, te.created_at, COALESCE(te.seq, 0)
             FROM task_events te
             LEFT JOIN tasks t ON t.id = te.task_id
             WHERE t.board_id = ?1
             ORDER BY te.created_at DESC
             LIMIT ?2".to_string(),
            vec![Box::new(board_id.to_string()), Box::new(limit)],
        )
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| db_error(&e.to_string()))?;

    let mut items: Vec<BoardActivityItem> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            let data_str: String = row.get(5)?;
            let data: serde_json::Value = serde_json::from_str(&data_str).unwrap_or(serde_json::json!({}));
            let mentions = data.get("mentions")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
            Ok(BoardActivityItem {
                id: row.get(0)?,
                task_id: row.get(1)?,
                task_title: row.get(2)?,
                event_type: row.get(3)?,
                actor: row.get(4)?,
                data,
                created_at: row.get(6)?,
                seq: row.get(7)?,
                task: None,
                recent_comments: None,
                mentions,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    // Filter by @mention if requested
    if let Some(mention_name) = mentioned {
        let mention_lower = mention_name.to_lowercase();
        items.retain(|item| {
            if let Some(ref mentions) = item.mentions {
                if mentions.iter().any(|m| m.to_lowercase() == mention_lower) {
                    return true;
                }
            }
            if item.actor.to_lowercase() == mention_lower {
                return true;
            }
            false
        });
    }

    // Enrich created/comment events with task snapshot and recent comments.
    let enrich_task_ids: Vec<String> = items
        .iter()
        .filter(|i| i.event_type == "created" || i.event_type == "comment")
        .map(|i| i.task_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if !enrich_task_ids.is_empty() {
        let placeholders: String = enrich_task_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect::<Vec<_>>().join(",");

        let task_sql = format!(
            "SELECT t.id, t.board_id, t.column_id, c.name, t.title, t.description,
                    t.priority, t.position, t.created_by, t.assigned_to, t.claimed_by,
                    t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at, t.archived_at,
                    t.created_at, t.updated_at,
                    (SELECT COUNT(*) FROM task_events te WHERE te.task_id = t.id AND te.event_type = 'comment') as comment_count
             FROM tasks t
             JOIN columns c ON t.column_id = c.id
             WHERE t.id IN ({})", placeholders
        );

        let task_params: Vec<Box<dyn rusqlite::types::ToSql>> = enrich_task_ids.iter().map(|id| Box::new(id.clone()) as Box<dyn rusqlite::types::ToSql>).collect();
        let task_param_refs: Vec<&dyn rusqlite::types::ToSql> = task_params.iter().map(|p| p.as_ref()).collect();

        let mut task_stmt = conn.prepare(&task_sql).map_err(|e| db_error(&e.to_string()))?;
        let task_map: std::collections::HashMap<String, TaskResponse> = task_stmt
            .query_map(task_param_refs.as_slice(), row_to_task)
            .map_err(|e| db_error(&e.to_string()))?
            .filter_map(|r| r.ok())
            .map(|t| (t.id.clone(), t))
            .collect();

        let comment_task_ids: Vec<String> = items.iter()
            .filter(|i| i.event_type == "comment")
            .map(|i| i.task_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let mut comments_map: std::collections::HashMap<String, Vec<CommentSnapshot>> = std::collections::HashMap::new();

        for tid in &comment_task_ids {
            let mut cmt_stmt = conn.prepare(
                "SELECT id, actor, data, created_at FROM task_events WHERE task_id = ?1 AND event_type = 'comment' ORDER BY created_at DESC LIMIT 10",
            ).map_err(|e| db_error(&e.to_string()))?;

            let cmts: Vec<CommentSnapshot> = cmt_stmt
                .query_map(rusqlite::params![tid], |row| {
                    let data_str: String = row.get(2)?;
                    let data_val: serde_json::Value = serde_json::from_str(&data_str).unwrap_or(serde_json::json!({}));
                    let message = data_val.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
                    Ok(CommentSnapshot { id: row.get(0)?, actor: row.get(1)?, message, created_at: row.get(3)? })
                })
                .map_err(|e| db_error(&e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();

            comments_map.insert(tid.clone(), cmts);
        }

        for item in &mut items {
            if item.event_type == "created" || item.event_type == "comment" {
                item.task = task_map.get(&item.task_id).cloned();
            }
            if item.event_type == "comment" {
                item.recent_comments = comments_map.remove(&item.task_id).or(Some(vec![]));
            }
        }
    }

    Ok(Json(items))
}

/// Get task events — public, no auth required.
#[get("/boards/<board_id>/tasks/<task_id>/events")]
pub fn get_task_events(
    board_id: &str,
    task_id: &str,
    db: &State<DbPool>,
) -> Result<Json<Vec<TaskEventResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_board_exists(&conn, board_id)?;

    let mut stmt = conn.prepare(
        "SELECT id, event_type, actor, data, created_at FROM task_events WHERE task_id = ?1 ORDER BY created_at ASC",
    ).map_err(|e| db_error(&e.to_string()))?;

    let events = stmt
        .query_map(rusqlite::params![task_id], |row| {
            let data_str: String = row.get(3)?;
            Ok(TaskEventResponse {
                id: row.get(0)?,
                event_type: row.get(1)?,
                actor: row.get(2)?,
                data: serde_json::from_str(&data_str).unwrap_or(serde_json::json!({})),
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(events))
}
