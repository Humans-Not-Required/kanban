use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool, DbPoolExt};
use crate::models::*;
use crate::rate_limit::{ClientIp, RateLimiter};

use super::{db_error, load_board_response};

/// Create a board — no auth required. Returns a manage_key (shown only once).
/// Rate limited per IP address to prevent spam.
#[post("/boards", format = "json", data = "<req>")]
pub fn create_board(
    req: Json<CreateBoardRequest>,
    client_ip: ClientIp,
    rate_limiter: &State<RateLimiter>,
    db: &State<DbPool>,
) -> Result<Json<CreateBoardResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();

    // Check IP-based rate limit for board creation
    let rl_result = rate_limiter.check_default(&client_ip.0);
    if !rl_result.allowed {
        return Err((
            Status::TooManyRequests,
            Json(ApiError {
                error: format!(
                    "Rate limit exceeded. You can create {} boards per hour. Try again in {} seconds.",
                    rl_result.limit, rl_result.reset_secs
                ),
                code: "RATE_LIMIT_EXCEEDED".to_string(),
                status: 429,
            }),
        ));
    }

    if req.name.trim().is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Board name cannot be empty".to_string(),
                code: "EMPTY_NAME".to_string(),
                status: 400,
            }),
        ));
    }

    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key = format!("kb_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let manage_key_hash = hash_key(&manage_key);

    let conn = db.conn();

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash, is_public, require_display_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![board_id, req.name.trim(), req.description, manage_key_hash, req.is_public as i32, req.require_display_name as i32],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    // Create default columns if none specified
    let columns = if req.columns.is_empty() {
        vec![
            "Backlog".to_string(),
            "Up Next".to_string(),
            "In Progress".to_string(),
            "Review".to_string(),
            "Done".to_string(),
        ]
    } else {
        req.columns
    };

    let mut col_responses = Vec::new();
    for (i, col_name) in columns.iter().enumerate() {
        let col_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![col_id, board_id, col_name, i as i32],
        )
        .map_err(|e| db_error(&e.to_string()))?;

        col_responses.push(ColumnResponse {
            id: col_id,
            name: col_name.clone(),
            position: i as i32,
            wip_limit: None,
            task_count: 0,
        });
    }

    Ok(Json(CreateBoardResponse {
        id: board_id.clone(),
        name: req.name,
        description: req.description,
        columns: col_responses,
        manage_key: manage_key.clone(),
        view_url: format!("/board/{}", board_id),
        manage_url: format!("/board/{}?key={}", board_id, manage_key),
        api_base: format!("/api/v1/boards/{}", board_id),
        created_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// List boards — public boards only (unless authenticated, future feature).
#[get("/boards?<include_archived>")]
pub fn list_boards(
    include_archived: Option<bool>,
    db: &State<DbPool>,
) -> Result<Json<Vec<BoardSummary>>, (Status, Json<ApiError>)> {
    let conn = db.conn();
    let show_archived = include_archived.unwrap_or(false);

    let archive_filter = if show_archived {
        ""
    } else {
        " AND b.archived = 0"
    };

    // Only show public boards in the listing
    let sql = format!(
        "SELECT b.id, b.name, b.description, b.archived, b.is_public, b.created_at,
                (SELECT COUNT(*) FROM tasks t WHERE t.board_id = b.id)
         FROM boards b
         WHERE b.is_public = 1{}
         ORDER BY b.created_at DESC",
        archive_filter
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| db_error(&e.to_string()))?;

    let boards: Vec<BoardSummary> = stmt
        .query_map([], |row| {
            Ok(BoardSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                archived: row.get::<_, i32>(3)? == 1,
                is_public: row.get::<_, i32>(4)? == 1,
                created_at: row.get(5)?,
                task_count: row.get(6)?,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(boards))
}

/// Update board name, description, or public flag — requires manage key.
#[patch("/boards/<board_id>", format = "json", data = "<req>")]
pub fn update_board(
    board_id: &str,
    req: Json<UpdateBoardRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let conn = db.conn();
    let token_hash = hash_key(&token.0);
    access::require_board_exists(&conn, board_id)?;
    access::require_manage_key(&conn, board_id, &token_hash)?;

    // Build dynamic update
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref name) = req.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err((Status::BadRequest, Json(ApiError {
                error: "Board name cannot be empty".to_string(),
                code: "INVALID_INPUT".to_string(),
                status: 400,
            })));
        }
        updates.push("name = ?");
        params.push(Box::new(trimmed.to_string()));
    }
    if let Some(ref desc) = req.description {
        updates.push("description = ?");
        params.push(Box::new(desc.trim().to_string()));
    }
    if let Some(is_public) = req.is_public {
        updates.push("is_public = ?");
        params.push(Box::new(is_public as i32));
    }
    if let Some(ref col_id) = req.quick_done_column_id {
        if col_id.is_empty() {
            updates.push("quick_done_column_id = NULL");
        } else {
            let col_exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM columns WHERE id = ?1 AND board_id = ?2",
                    rusqlite::params![col_id, board_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if !col_exists {
                return Err((Status::BadRequest, Json(ApiError {
                    error: "quick_done_column_id must reference a column on this board".to_string(),
                    code: "INVALID_COLUMN".to_string(),
                    status: 400,
                })));
            }
            updates.push("quick_done_column_id = ?");
            params.push(Box::new(col_id.clone()));
        }
    }
    if let Some(auto_archive) = req.quick_done_auto_archive {
        updates.push("quick_done_auto_archive = ?");
        params.push(Box::new(auto_archive as i32));
    }
    if let Some(ref col_id) = req.quick_reassign_column_id {
        if col_id.is_empty() {
            updates.push("quick_reassign_column_id = NULL");
        } else {
            let col_exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM columns WHERE id = ?1 AND board_id = ?2",
                    rusqlite::params![col_id, board_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if !col_exists {
                return Err((Status::BadRequest, Json(ApiError {
                    error: "quick_reassign_column_id must reference a column on this board".to_string(),
                    code: "INVALID_COLUMN".to_string(),
                    status: 400,
                })));
            }
            updates.push("quick_reassign_column_id = ?");
            params.push(Box::new(col_id.clone()));
        }
    }
    if let Some(ref reassign_to) = req.quick_reassign_to {
        if reassign_to.is_empty() {
            updates.push("quick_reassign_to = NULL");
        } else {
            updates.push("quick_reassign_to = ?");
            params.push(Box::new(reassign_to.trim().to_string()));
        }
    }
    if let Some(require_display_name) = req.require_display_name {
        updates.push("require_display_name = ?");
        params.push(Box::new(require_display_name as i32));
    }

    if updates.is_empty() {
        return load_board_response(&conn, board_id);
    }

    updates.push("updated_at = datetime('now')");
    let sql = format!("UPDATE boards SET {} WHERE id = ?", updates.join(", "));
    params.push(Box::new(board_id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| db_error(&e.to_string()))?;

    load_board_response(&conn, board_id)
}

/// Archive a board — requires manage key.
#[post("/boards/<board_id>/archive")]
pub fn archive_board(
    board_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let conn = db.conn();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let already_archived: bool = conn
        .query_row(
            "SELECT archived = 1 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if already_archived {
        return Err((
            Status::Conflict,
            Json(ApiError {
                error: "Board is already archived".to_string(),
                code: "ALREADY_ARCHIVED".to_string(),
                status: 409,
            }),
        ));
    }

    conn.execute(
        "UPDATE boards SET archived = 1, updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    load_board_response(&conn, board_id)
}

/// Unarchive a board — requires manage key.
#[post("/boards/<board_id>/unarchive")]
pub fn unarchive_board(
    board_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let conn = db.conn();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let is_archived: bool = conn
        .query_row(
            "SELECT archived = 1 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !is_archived {
        return Err((
            Status::Conflict,
            Json(ApiError {
                error: "Board is not archived".to_string(),
                code: "NOT_ARCHIVED".to_string(),
                status: 409,
            }),
        ));
    }

    conn.execute(
        "UPDATE boards SET archived = 0, updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    load_board_response(&conn, board_id)
}

/// Get board details — public, no auth required. Anyone with the UUID can view.
#[get("/boards/<board_id>")]
pub fn get_board(
    board_id: &str,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let conn = db.conn();
    load_board_response(&conn, board_id)
}
