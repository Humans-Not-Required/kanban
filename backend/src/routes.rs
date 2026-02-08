use std::path::PathBuf;

use rocket::http::{ContentType, Status};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::tokio::select;
use rocket::tokio::time::Duration;
use rocket::{Shutdown, State};

use crate::access;
use crate::auth::BoardToken;
use crate::db::{hash_key, DbPool};
use crate::events::EventBus;
use crate::models::*;
use crate::rate_limit::{ClientIp, RateLimiter};

// ============ Health & OpenAPI ============

#[get("/health")]
pub fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[get("/openapi.json")]
pub fn openapi() -> (ContentType, &'static str) {
    (ContentType::JSON, include_str!("../openapi.json"))
}

// ============ SSE Event Stream ============

/// Public: anyone with the board UUID can subscribe to events.
#[get("/boards/<board_id>/events/stream")]
pub fn board_event_stream(
    board_id: &str,
    db: &State<DbPool>,
    bus: &State<EventBus>,
    mut shutdown: Shutdown,
) -> Result<EventStream![], (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_board_exists(&conn, board_id)?;
    drop(conn);

    let mut rx = bus.subscribe(board_id);

    Ok(EventStream! {
        loop {
            select! {
                msg = rx.recv() => match msg {
                    Ok(event) => {
                        yield Event::json(&event.data).event(event.event);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        yield Event::data("events_lost").event("warning".to_string());
                    }
                },
                _ = &mut shutdown => break,
            }
        }
    }
    .heartbeat(Duration::from_secs(15)))
}

// ============ Boards ============

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

    let conn = db.lock().unwrap();

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash, is_public) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![board_id, req.name.trim(), req.description, manage_key_hash, req.is_public as i32],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    // Create default columns if none specified
    let columns = if req.columns.is_empty() {
        vec![
            "Backlog".to_string(),
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
    let conn = db.lock().unwrap();
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

// ============ Board Archive / Unarchive ============

/// Archive a board — requires manage key.
#[post("/boards/<board_id>/archive")]
pub fn archive_board(
    board_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
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
    let conn = db.lock().unwrap();
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
    let conn = db.lock().unwrap();
    load_board_response(&conn, board_id)
}

// ============ Columns ============

/// Create a column — requires manage key.
#[post("/boards/<board_id>/columns", format = "json", data = "<req>")]
pub fn create_column(
    board_id: &str,
    req: Json<CreateColumnRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<ColumnResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

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
    let conn = db.lock().unwrap();

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
    let conn = db.lock().unwrap();

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
    let conn = db.lock().unwrap();

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

// ============ Tasks ============

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
    let conn = db.lock().unwrap();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    if req.title.trim().is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Task title cannot be empty".to_string(),
                code: "EMPTY_TITLE".to_string(),
                status: 400,
            }),
        ));
    }

    // Resolve column: use provided ID, or first column of the board
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

    // Check WIP limit
    check_wip_limit(&conn, &column_id, None)?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let creator = if req.actor_name.is_empty() {
        "anonymous".to_string()
    } else {
        req.actor_name.clone()
    };
    let labels_json = serde_json::to_string(&req.labels).unwrap_or_else(|_| "[]".to_string());
    let metadata_json = serde_json::to_string(&req.metadata).unwrap_or_else(|_| "{}".to_string());

    // Determine position
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
            task_id,
            board_id,
            column_id,
            req.title.trim(),
            req.description,
            req.priority,
            position,
            creator,
            req.assigned_to,
            labels_json,
            metadata_json,
            req.due_at,
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

/// Search tasks — public, no auth required.
#[allow(clippy::too_many_arguments)]
#[get(
    "/boards/<board_id>/tasks/search?<q>&<column>&<assigned>&<priority>&<label>&<limit>&<offset>"
)]
pub fn search_tasks(
    board_id: &str,
    q: &str,
    column: Option<&str>,
    assigned: Option<&str>,
    priority: Option<i32>,
    label: Option<&str>,
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
                t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at,
                t.created_at, t.updated_at
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

    // Count total matches
    let count_sql = sql.replace(
        "SELECT t.id, t.board_id, t.column_id, c.name, t.title, t.description,
                t.priority, t.position, t.created_by, t.assigned_to, t.claimed_by,
                t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at,
                t.created_at, t.updated_at",
        "SELECT COUNT(*)",
    );
    let count_param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let total: i64 = conn
        .query_row(&count_sql, count_param_refs.as_slice(), |row| row.get(0))
        .unwrap_or(0);

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

    Ok(Json(SearchResponse {
        query: query.to_string(),
        tasks,
        total,
        limit,
        offset,
    }))
}

/// List tasks — public, no auth required.
#[allow(clippy::too_many_arguments)]
#[get("/boards/<board_id>/tasks?<column>&<assigned>&<claimed>&<priority>&<label>")]
pub fn list_tasks(
    board_id: &str,
    column: Option<&str>,
    assigned: Option<&str>,
    claimed: Option<&str>,
    priority: Option<i32>,
    label: Option<&str>,
    db: &State<DbPool>,
) -> Result<Json<Vec<TaskResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_board_exists(&conn, board_id)?;

    let mut sql = String::from(
        "SELECT t.id, t.board_id, t.column_id, c.name, t.title, t.description,
                t.priority, t.position, t.created_by, t.assigned_to, t.claimed_by,
                t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at,
                t.created_at, t.updated_at
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

    sql.push_str(" ORDER BY c.position ASC, t.priority DESC, t.position ASC");

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
    let conn = db.lock().unwrap();
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
    let conn = db.lock().unwrap();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;
    let _existing = load_task_response(&conn, task_id)?;
    let actor = req.actor_name.clone().unwrap_or_else(|| "anonymous".to_string());
    let mut changes = serde_json::Map::new();

    if let Some(ref title) = req.title {
        conn.execute(
            "UPDATE tasks SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![title, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
        changes.insert("title".into(), serde_json::json!(title));
    }

    if let Some(ref desc) = req.description {
        conn.execute(
            "UPDATE tasks SET description = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![desc, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
        changes.insert("description".into(), serde_json::json!(desc));
    }

    if let Some(ref col_id) = req.column_id {
        check_wip_limit(&conn, col_id, Some(task_id))?;
        conn.execute(
            "UPDATE tasks SET column_id = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![col_id, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
        changes.insert("column_id".into(), serde_json::json!(col_id));
    }

    if let Some(p) = req.priority {
        conn.execute(
            "UPDATE tasks SET priority = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![p, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
        changes.insert("priority".into(), serde_json::json!(p));
    }

    if let Some(ref assigned) = req.assigned_to {
        conn.execute(
            "UPDATE tasks SET assigned_to = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![assigned, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
        changes.insert("assigned_to".into(), serde_json::json!(assigned));
    }

    if let Some(ref labels) = req.labels {
        let labels_json = serde_json::to_string(labels).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "UPDATE tasks SET labels = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![labels_json, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
        changes.insert("labels".into(), serde_json::json!(labels));
    }

    if let Some(ref meta) = req.metadata {
        let meta_json = serde_json::to_string(meta).unwrap_or_else(|_| "{}".to_string());
        conn.execute(
            "UPDATE tasks SET metadata = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![meta_json, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
        changes.insert("metadata".into(), meta.clone());
    }

    if let Some(ref due) = req.due_at {
        conn.execute(
            "UPDATE tasks SET due_at = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![due, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
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
#[delete("/boards/<board_id>/tasks/<task_id>")]
pub fn delete_task(
    board_id: &str,
    task_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    let affected = conn
        .execute(
            "DELETE FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![task_id, board_id],
        )
        .unwrap_or(0);

    if affected > 0 {
        bus.emit(crate::events::BoardEvent {
            event: "task.deleted".to_string(),
            board_id: board_id.to_string(),
            data: serde_json::json!({"task_id": task_id}),
        });
        Ok(Json(serde_json::json!({"deleted": true, "id": task_id})))
    } else {
        Err(not_found("Task"))
    }
}

// ============ Agent-First: Claim / Release ============

/// Claim a task — requires manage key.
#[post("/boards/<board_id>/tasks/<task_id>/claim?<agent>")]
pub fn claim_task(
    board_id: &str,
    task_id: &str,
    agent: Option<&str>,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    let actor = agent.unwrap_or("anonymous").to_string();

    // Check if already claimed by someone else
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

    let event_data = serde_json::json!({"task_id": task_id, "agent": actor});
    log_event(&conn, task_id, "claimed", &actor, &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.claimed".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

/// Release a claimed task — requires manage key.
#[post("/boards/<board_id>/tasks/<task_id>/release")]
pub fn release_task(
    board_id: &str,
    task_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    conn.execute(
        "UPDATE tasks SET claimed_by = NULL, claimed_at = NULL, updated_at = datetime('now') WHERE id = ?1 AND board_id = ?2",
        rusqlite::params![task_id, board_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let event_data = serde_json::json!({"task_id": task_id});
    log_event(&conn, task_id, "released", "anonymous", &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.released".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

/// Move a task to a different column — requires manage key.
#[post("/boards/<board_id>/tasks/<task_id>/move/<target_column_id>")]
pub fn move_task(
    board_id: &str,
    task_id: &str,
    target_column_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;
    access::require_not_archived(&conn, board_id)?;

    // Verify target column belongs to the board
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

    let event_data = serde_json::json!({"task_id": task_id, "from": from_col, "to": target_column_id});
    log_event(&conn, task_id, "moved", "anonymous", &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.moved".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

// ============ Task Reorder ============

/// Reorder a task — requires manage key.
#[post(
    "/boards/<board_id>/tasks/<task_id>/reorder",
    format = "json",
    data = "<req>"
)]
pub fn reorder_task(
    board_id: &str,
    task_id: &str,
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

        let completed = if is_done_column {
            "datetime('now')"
        } else {
            "NULL"
        };

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
    log_event(&conn, task_id, "reordered", "anonymous", &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.reordered".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

// ============ Batch Operations ============

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

    if req.operations.is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "No operations provided".to_string(),
                code: "EMPTY_BATCH".to_string(),
                status: 400,
            }),
        ));
    }

    if req.operations.len() > 50 {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Maximum 50 operations per batch request".to_string(),
                code: "BATCH_TOO_LARGE".to_string(),
                status: 400,
            }),
        ));
    }

    let mut results = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;

    for op in &req.operations {
        match op {
            BatchOperation::Move {
                task_ids,
                column_id,
            } => {
                let result = batch_move(&conn, board_id, task_ids, column_id, bus);
                match result {
                    Ok(affected) => {
                        succeeded += 1;
                        results.push(BatchOperationResult {
                            action: "move".to_string(),
                            task_ids: task_ids.clone(),
                            success: true,
                            error: None,
                            affected,
                        });
                    }
                    Err(msg) => {
                        failed += 1;
                        results.push(BatchOperationResult {
                            action: "move".to_string(),
                            task_ids: task_ids.clone(),
                            success: false,
                            error: Some(msg),
                            affected: 0,
                        });
                    }
                }
            }
            BatchOperation::Update { task_ids, fields } => {
                let result = batch_update(&conn, board_id, task_ids, fields, bus);
                match result {
                    Ok(affected) => {
                        succeeded += 1;
                        results.push(BatchOperationResult {
                            action: "update".to_string(),
                            task_ids: task_ids.clone(),
                            success: true,
                            error: None,
                            affected,
                        });
                    }
                    Err(msg) => {
                        failed += 1;
                        results.push(BatchOperationResult {
                            action: "update".to_string(),
                            task_ids: task_ids.clone(),
                            success: false,
                            error: Some(msg),
                            affected: 0,
                        });
                    }
                }
            }
            BatchOperation::Delete { task_ids } => {
                let result = batch_delete(&conn, board_id, task_ids, bus);
                match result {
                    Ok(affected) => {
                        succeeded += 1;
                        results.push(BatchOperationResult {
                            action: "delete".to_string(),
                            task_ids: task_ids.clone(),
                            success: true,
                            error: None,
                            affected,
                        });
                    }
                    Err(msg) => {
                        failed += 1;
                        results.push(BatchOperationResult {
                            action: "delete".to_string(),
                            task_ids: task_ids.clone(),
                            success: false,
                            error: Some(msg),
                            affected: 0,
                        });
                    }
                }
            }
        }
    }

    Ok(Json(BatchResponse {
        total: req.operations.len(),
        succeeded,
        failed,
        results,
    }))
}

fn batch_move(
    conn: &Connection,
    board_id: &str,
    task_ids: &[String],
    column_id: &str,
    bus: &EventBus,
) -> Result<usize, String> {
    let col_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM columns WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![column_id, board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !col_exists {
        return Err("Target column not found in this board".to_string());
    }

    let is_done_column: bool = conn
        .query_row(
            "SELECT position = (SELECT MAX(position) FROM columns WHERE board_id = ?1) FROM columns WHERE id = ?2",
            rusqlite::params![board_id, column_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    let mut affected = 0;
    for task_id in task_ids {
        let belongs: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2",
                rusqlite::params![task_id, board_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !belongs {
            continue;
        }

        let from_col: String = conn
            .query_row(
                "SELECT column_id FROM tasks WHERE id = ?1",
                rusqlite::params![task_id],
                |row| row.get(0),
            )
            .unwrap_or_default();

        let rows = if is_done_column {
            conn.execute(
                "UPDATE tasks SET column_id = ?1, completed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?2 AND board_id = ?3",
                rusqlite::params![column_id, task_id, board_id],
            )
            .unwrap_or(0)
        } else {
            conn.execute(
                "UPDATE tasks SET column_id = ?1, completed_at = NULL, updated_at = datetime('now') WHERE id = ?2 AND board_id = ?3",
                rusqlite::params![column_id, task_id, board_id],
            )
            .unwrap_or(0)
        };

        if rows > 0 {
            affected += 1;
            let event_data = serde_json::json!({"task_id": task_id, "from": from_col, "to": column_id, "batch": true});
            log_event(conn, task_id, "moved", "batch", &event_data);
            bus.emit(crate::events::BoardEvent {
                event: "task.moved".to_string(),
                board_id: board_id.to_string(),
                data: event_data,
            });
        }
    }

    Ok(affected)
}

fn batch_update(
    conn: &Connection,
    board_id: &str,
    task_ids: &[String],
    fields: &BatchUpdateFields,
    bus: &EventBus,
) -> Result<usize, String> {
    let mut affected = 0;

    for task_id in task_ids {
        let belongs: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2",
                rusqlite::params![task_id, board_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !belongs {
            continue;
        }

        let mut changes = serde_json::Map::new();

        if let Some(p) = fields.priority {
            conn.execute(
                "UPDATE tasks SET priority = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![p, task_id],
            )
            .ok();
            changes.insert("priority".into(), serde_json::json!(p));
        }

        if let Some(ref assigned) = fields.assigned_to {
            conn.execute(
                "UPDATE tasks SET assigned_to = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![assigned, task_id],
            )
            .ok();
            changes.insert("assigned_to".into(), serde_json::json!(assigned));
        }

        if let Some(ref labels) = fields.labels {
            let labels_json = serde_json::to_string(labels).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "UPDATE tasks SET labels = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![labels_json, task_id],
            )
            .ok();
            changes.insert("labels".into(), serde_json::json!(labels));
        }

        if let Some(ref due) = fields.due_at {
            conn.execute(
                "UPDATE tasks SET due_at = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![due, task_id],
            )
            .ok();
            changes.insert("due_at".into(), serde_json::json!(due));
        }

        if !changes.is_empty() {
            affected += 1;
            let event_data = serde_json::Value::Object(changes.clone());
            log_event(conn, task_id, "updated", "batch", &event_data);

            let mut emit_data = changes;
            emit_data.insert("task_id".into(), serde_json::json!(task_id));
            emit_data.insert("batch".into(), serde_json::json!(true));
            bus.emit(crate::events::BoardEvent {
                event: "task.updated".to_string(),
                board_id: board_id.to_string(),
                data: serde_json::Value::Object(emit_data),
            });
        }
    }

    Ok(affected)
}

fn batch_delete(
    conn: &Connection,
    board_id: &str,
    task_ids: &[String],
    bus: &EventBus,
) -> Result<usize, String> {
    let mut affected = 0;

    for task_id in task_ids {
        let rows = conn
            .execute(
                "DELETE FROM tasks WHERE id = ?1 AND board_id = ?2",
                rusqlite::params![task_id, board_id],
            )
            .unwrap_or(0);

        if rows > 0 {
            affected += 1;
            bus.emit(crate::events::BoardEvent {
                event: "task.deleted".to_string(),
                board_id: board_id.to_string(),
                data: serde_json::json!({"task_id": task_id, "batch": true}),
            });
        }
    }

    Ok(affected)
}

// ============ Task Events ============

/// Get task events — public, no auth required.
#[get("/boards/<board_id>/tasks/<task_id>/events")]
pub fn get_task_events(
    board_id: &str,
    task_id: &str,
    db: &State<DbPool>,
) -> Result<Json<Vec<TaskEventResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_board_exists(&conn, board_id)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, event_type, actor, data, created_at
             FROM task_events WHERE task_id = ?1
             ORDER BY created_at ASC",
        )
        .map_err(|e| db_error(&e.to_string()))?;

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
    let data = serde_json::json!({"message": message, "actor": actor});
    let data_str = serde_json::to_string(&data).unwrap();

    conn.execute(
        "INSERT INTO task_events (id, task_id, event_type, actor, data) VALUES (?1, ?2, 'comment', ?3, ?4)",
        rusqlite::params![event_id, task_id, actor, data_str],
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
        data: serde_json::json!({"task_id": task_id, "actor": &actor, "message": message}),
    });

    Ok(Json(TaskEventResponse {
        id: event_id,
        event_type: "comment".to_string(),
        actor,
        data,
        created_at,
    }))
}

// ============ Webhooks ============

/// Create a webhook — requires manage key.
#[post("/boards/<board_id>/webhooks", format = "json", data = "<req>")]
pub fn create_webhook(
    board_id: &str,
    req: Json<CreateWebhookRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<WebhookResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    if req.url.trim().is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Webhook URL cannot be empty".to_string(),
                code: "EMPTY_URL".to_string(),
                status: 400,
            }),
        ));
    }

    let valid_events = [
        "task.created",
        "task.updated",
        "task.deleted",
        "task.claimed",
        "task.released",
        "task.moved",
        "task.reordered",
        "task.comment",
        "task.dependency.added",
        "task.dependency.removed",
    ];
    for ev in &req.events {
        if !valid_events.contains(&ev.as_str()) {
            return Err((
                Status::BadRequest,
                Json(ApiError {
                    error: format!(
                        "Invalid event type '{}'. Valid types: {}",
                        ev,
                        valid_events.join(", ")
                    ),
                    code: "INVALID_EVENT_TYPE".to_string(),
                    status: 400,
                }),
            ));
        }
    }

    let webhook_id = uuid::Uuid::new_v4().to_string();
    let secret = format!(
        "whsec_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")
    );
    let events_json = serde_json::to_string(&req.events).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO webhooks (id, board_id, url, secret, events) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![webhook_id, board_id, req.url.trim(), secret, events_json],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    Ok(Json(WebhookResponse {
        id: webhook_id,
        board_id: board_id.to_string(),
        url: req.url,
        secret: Some(secret),
        events: req.events,
        active: true,
        failure_count: 0,
        last_triggered_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// List webhooks — requires manage key.
#[get("/boards/<board_id>/webhooks")]
pub fn list_webhooks(
    board_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<Vec<WebhookResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, board_id, url, events, active, failure_count, last_triggered_at, created_at
             FROM webhooks WHERE board_id = ?1
             ORDER BY created_at ASC",
        )
        .map_err(|e| db_error(&e.to_string()))?;

    let webhooks: Vec<WebhookResponse> = stmt
        .query_map(rusqlite::params![board_id], |row| {
            let events_str: String = row.get(3)?;
            let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
            Ok(WebhookResponse {
                id: row.get(0)?,
                board_id: row.get(1)?,
                url: row.get(2)?,
                secret: None,
                events,
                active: row.get::<_, i32>(4)? == 1,
                failure_count: row.get(5)?,
                last_triggered_at: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(webhooks))
}

/// Update a webhook — requires manage key.
#[patch(
    "/boards/<board_id>/webhooks/<webhook_id>",
    format = "json",
    data = "<req>"
)]
pub fn update_webhook(
    board_id: &str,
    webhook_id: &str,
    req: Json<UpdateWebhookRequest>,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<WebhookResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM webhooks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![webhook_id, board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !exists {
        return Err(not_found("Webhook"));
    }

    if let Some(ref url) = req.url {
        if url.trim().is_empty() {
            return Err((
                Status::BadRequest,
                Json(ApiError {
                    error: "Webhook URL cannot be empty".to_string(),
                    code: "EMPTY_URL".to_string(),
                    status: 400,
                }),
            ));
        }
        conn.execute(
            "UPDATE webhooks SET url = ?1 WHERE id = ?2",
            rusqlite::params![url.trim(), webhook_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    }

    if let Some(ref events) = req.events {
        let valid_events = [
            "task.created",
            "task.updated",
            "task.deleted",
            "task.claimed",
            "task.released",
            "task.moved",
            "task.reordered",
            "task.comment",
            "task.dependency.added",
            "task.dependency.removed",
        ];
        for ev in events {
            if !valid_events.contains(&ev.as_str()) {
                return Err((
                    Status::BadRequest,
                    Json(ApiError {
                        error: format!("Invalid event type '{}'", ev),
                        code: "INVALID_EVENT_TYPE".to_string(),
                        status: 400,
                    }),
                ));
            }
        }
        let events_json = serde_json::to_string(events).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "UPDATE webhooks SET events = ?1 WHERE id = ?2",
            rusqlite::params![events_json, webhook_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    }

    if let Some(active) = req.active {
        let active_int: i32 = if active { 1 } else { 0 };
        if active {
            conn.execute(
                "UPDATE webhooks SET active = ?1, failure_count = 0 WHERE id = ?2",
                rusqlite::params![active_int, webhook_id],
            )
            .map_err(|e| db_error(&e.to_string()))?;
        } else {
            conn.execute(
                "UPDATE webhooks SET active = ?1 WHERE id = ?2",
                rusqlite::params![active_int, webhook_id],
            )
            .map_err(|e| db_error(&e.to_string()))?;
        }
    }

    let wh = conn
        .query_row(
            "SELECT id, board_id, url, events, active, failure_count, last_triggered_at, created_at
             FROM webhooks WHERE id = ?1",
            rusqlite::params![webhook_id],
            |row| {
                let events_str: String = row.get(3)?;
                let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
                Ok(WebhookResponse {
                    id: row.get(0)?,
                    board_id: row.get(1)?,
                    url: row.get(2)?,
                    secret: None,
                    events,
                    active: row.get::<_, i32>(4)? == 1,
                    failure_count: row.get(5)?,
                    last_triggered_at: row.get(6)?,
                    created_at: row.get(7)?,
                })
            },
        )
        .map_err(|_| not_found("Webhook"))?;

    Ok(Json(wh))
}

/// Delete a webhook — requires manage key.
#[delete("/boards/<board_id>/webhooks/<webhook_id>")]
pub fn delete_webhook(
    board_id: &str,
    webhook_id: &str,
    token: BoardToken,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_key(&token.0);
    access::require_manage_key(&conn, board_id, &token_hash)?;

    let affected = conn
        .execute(
            "DELETE FROM webhooks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![webhook_id, board_id],
        )
        .unwrap_or(0);

    if affected > 0 {
        Ok(Json(serde_json::json!({"deleted": true, "id": webhook_id})))
    } else {
        Err(not_found("Webhook"))
    }
}

// ============ Task Dependencies ============

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
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "A task cannot depend on itself".to_string(),
                code: "SELF_DEPENDENCY".to_string(),
                status: 400,
            }),
        ));
    }

    let blocker_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![req.blocker_task_id, board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    let blocked_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![req.blocked_task_id, board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !blocker_exists {
        return Err(not_found("Blocker task"));
    }
    if !blocked_exists {
        return Err(not_found("Blocked task"));
    }

    let reverse_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM task_dependencies WHERE blocker_task_id = ?1 AND blocked_task_id = ?2",
            rusqlite::params![req.blocked_task_id, req.blocker_task_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if reverse_exists {
        return Err((
            Status::Conflict,
            Json(ApiError {
                error: "Circular dependency: the reverse relationship already exists".to_string(),
                code: "CIRCULAR_DEPENDENCY".to_string(),
                status: 409,
            }),
        ));
    }

    if has_path(&conn, &req.blocked_task_id, &req.blocker_task_id) {
        return Err((
            Status::Conflict,
            Json(ApiError {
                error: "Circular dependency: this would create a cycle in the dependency graph"
                    .to_string(),
                code: "CIRCULAR_DEPENDENCY".to_string(),
                status: 409,
            }),
        ));
    }

    let dep_id = uuid::Uuid::new_v4().to_string();
    let result = conn.execute(
        "INSERT INTO task_dependencies (id, board_id, blocker_task_id, blocked_task_id, note) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![dep_id, board_id, req.blocker_task_id, req.blocked_task_id, req.note],
    );

    match result {
        Ok(_) => {}
        Err(e) if e.to_string().contains("UNIQUE") => {
            return Err((
                Status::Conflict,
                Json(ApiError {
                    error: "This dependency already exists".to_string(),
                    code: "DUPLICATE_DEPENDENCY".to_string(),
                    status: 409,
                }),
            ));
        }
        Err(e) => return Err(db_error(&e.to_string())),
    }

    let event_data = serde_json::json!({
        "dependency_id": dep_id,
        "blocker_task_id": req.blocker_task_id,
        "blocked_task_id": req.blocked_task_id,
        "note": req.note,
    });
    log_event(
        &conn,
        &req.blocked_task_id,
        "dependency.added",
        "anonymous",
        &event_data,
    );

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

    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(task_id) = task
    {
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
            vec![
                Box::new(board_id.to_string()) as Box<dyn rusqlite::types::ToSql>,
                Box::new(task_id.to_string()),
            ],
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
                id: row.get(0)?,
                board_id: row.get(1)?,
                blocker_task_id: row.get(2)?,
                blocker_title: row.get(3)?,
                blocker_column: row.get(4)?,
                blocker_completed: row.get(5)?,
                blocked_task_id: row.get(6)?,
                blocked_title: row.get(7)?,
                blocked_column: row.get(8)?,
                note: row.get(9)?,
                created_by: row.get(10)?,
                created_at: row.get(11)?,
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
        .execute(
            "DELETE FROM task_dependencies WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![dep_id, board_id],
        )
        .unwrap_or(0);

    if affected > 0 {
        let event_data = serde_json::json!({
            "dependency_id": dep_id,
            "blocker_task_id": blocker_id,
            "blocked_task_id": blocked_id,
        });
        log_event(
            &conn,
            &blocked_id,
            "dependency.removed",
            "anonymous",
            &event_data,
        );

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

// ============ Helpers ============

fn has_path(conn: &Connection, from_task: &str, to_task: &str) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(from_task.to_string());

    while let Some(current) = queue.pop_front() {
        if current == to_task {
            return true;
        }
        if !visited.insert(current.clone()) {
            continue;
        }
        if let Ok(mut stmt) =
            conn.prepare("SELECT blocked_task_id FROM task_dependencies WHERE blocker_task_id = ?1")
        {
            if let Ok(rows) =
                stmt.query_map(rusqlite::params![current], |row| row.get::<_, String>(0))
            {
                for row in rows.flatten() {
                    if !visited.contains(&row) {
                        queue.push_back(row);
                    }
                }
            }
        }
    }
    false
}

fn load_dependency_response(
    conn: &Connection,
    dep_id: &str,
) -> Result<Json<DependencyResponse>, (Status, Json<ApiError>)> {
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
                id: row.get(0)?,
                board_id: row.get(1)?,
                blocker_task_id: row.get(2)?,
                blocker_title: row.get(3)?,
                blocker_column: row.get(4)?,
                blocker_completed: row.get(5)?,
                blocked_task_id: row.get(6)?,
                blocked_title: row.get(7)?,
                blocked_column: row.get(8)?,
                note: row.get(9)?,
                created_by: row.get(10)?,
                created_at: row.get(11)?,
            })
        },
    )
    .map(Json)
    .map_err(|_| not_found("Dependency"))
}

fn log_event(
    conn: &Connection,
    task_id: &str,
    event_type: &str,
    actor: &str,
    data: &serde_json::Value,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let data_str = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    let _ = conn.execute(
        "INSERT INTO task_events (id, task_id, event_type, actor, data) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, task_id, event_type, actor, data_str],
    );
}

fn load_board_response(
    conn: &Connection,
    board_id: &str,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let board = conn
        .query_row(
            "SELECT b.id, b.name, b.description, b.archived, b.is_public, b.created_at, b.updated_at
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
        created_at: board.5,
        updated_at: board.6,
    }))
}

fn load_task_response(
    conn: &Connection,
    task_id: &str,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    conn.query_row(
        "SELECT t.id, t.board_id, t.column_id, c.name, t.title, t.description,
                t.priority, t.position, t.created_by, t.assigned_to, t.claimed_by,
                t.claimed_at, t.labels, t.metadata, t.due_at, t.completed_at,
                t.created_at, t.updated_at
         FROM tasks t
         JOIN columns c ON t.column_id = c.id
         WHERE t.id = ?1",
        rusqlite::params![task_id],
        row_to_task,
    )
    .map(Json)
    .map_err(|_| not_found("Task"))
}

fn row_to_task(row: &rusqlite::Row) -> Result<TaskResponse, rusqlite::Error> {
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
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

use rusqlite::Connection;

fn db_error(msg: &str) -> (Status, Json<ApiError>) {
    (
        Status::InternalServerError,
        Json(ApiError {
            error: format!("Database error: {}", msg),
            code: "DB_ERROR".to_string(),
            status: 500,
        }),
    )
}

fn not_found(entity: &str) -> (Status, Json<ApiError>) {
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
fn check_wip_limit(
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

// ============ SPA Fallback ============

#[get("/<_path..>", rank = 20)]
pub fn spa_fallback(_path: PathBuf) -> Option<(ContentType, Vec<u8>)> {
    let static_dir: PathBuf = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));
    let index_path = static_dir.join("index.html");
    std::fs::read(&index_path)
        .ok()
        .map(|bytes| (ContentType::HTML, bytes))
}
