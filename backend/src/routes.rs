use std::path::PathBuf;

use rocket::http::{ContentType, Status};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::tokio::select;
use rocket::tokio::time::Duration;
use rocket::{Shutdown, State};

use crate::access::{self, BoardRole};
use crate::auth::AuthenticatedKey;
use crate::db::DbPool;
use crate::events::EventBus;
use crate::models::*;

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

#[get("/boards/<board_id>/events/stream")]
pub fn board_event_stream(
    board_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
    mut shutdown: Shutdown,
) -> Result<EventStream![], (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;
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

#[post("/boards", format = "json", data = "<req>")]
pub fn create_board(
    req: Json<CreateBoardRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();

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
    let conn = db.lock().unwrap();

    conn.execute(
        "INSERT INTO boards (id, name, description, owner_key_id) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![board_id, req.name.trim(), req.description, key.id],
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

    let owner = key.agent_id.unwrap_or_else(|| key.id.clone());

    Ok(Json(BoardResponse {
        id: board_id,
        name: req.name,
        description: req.description,
        owner,
        columns: col_responses,
        task_count: 0,
        archived: false,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }))
}

#[get("/boards?<include_archived>")]
pub fn list_boards(
    include_archived: Option<bool>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<BoardSummary>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let show_archived = include_archived.unwrap_or(false);

    // Admin keys see all boards; regular keys see boards they own, collaborate on, or have tasks in
    let archive_filter = if show_archived {
        ""
    } else {
        " AND b.archived = 0"
    };

    let (sql, param1, param2) = if key.is_admin {
        (
            format!(
                "SELECT b.id, b.name, b.description, b.archived, b.created_at,
                    (SELECT COUNT(*) FROM tasks t WHERE t.board_id = b.id)
                 FROM boards b
                 WHERE 1=1{}
                 ORDER BY b.created_at DESC",
                archive_filter
            ),
            String::new(),
            String::new(),
        )
    } else {
        let agent = key.agent_id.as_deref().unwrap_or(&key.id);
        (
            format!(
                "SELECT b.id, b.name, b.description, b.archived, b.created_at,
                    (SELECT COUNT(*) FROM tasks t WHERE t.board_id = b.id)
                 FROM boards b
                 WHERE (b.owner_key_id = ?1
                    OR b.id IN (SELECT board_id FROM board_collaborators WHERE key_id = ?1)
                    OR b.id IN (SELECT DISTINCT board_id FROM tasks WHERE created_by = ?1 OR assigned_to = ?2 OR claimed_by = ?2)){}
                 ORDER BY b.created_at DESC",
                archive_filter
            ),
            key.id.clone(),
            agent.to_string(),
        )
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| db_error(&e.to_string()))?;

    let board_mapper = |row: &rusqlite::Row| {
        Ok(BoardSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            archived: row.get::<_, i32>(3)? == 1,
            created_at: row.get(4)?,
            task_count: row.get(5)?,
        })
    };

    let boards: Vec<BoardSummary> = if key.is_admin {
        stmt.query_map([], board_mapper)
            .map_err(|e| db_error(&e.to_string()))?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        stmt.query_map(rusqlite::params![param1, param2], board_mapper)
            .map_err(|e| db_error(&e.to_string()))?
            .filter_map(|r| r.ok())
            .collect()
    };

    Ok(Json(boards))
}

// ============ Board Archive / Unarchive ============

/// Archive a board — prevents new task creation and modifications.
/// Requires Admin role on the board.
#[post("/boards/<board_id>/archive")]
pub fn archive_board(
    board_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;

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

/// Unarchive a board — restores normal operations.
/// Requires Admin role on the board.
#[post("/boards/<board_id>/unarchive")]
pub fn unarchive_board(
    board_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;

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

#[get("/boards/<board_id>")]
pub fn get_board(
    board_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<BoardResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;
    load_board_response(&conn, board_id)
}

// ============ Columns ============

#[post("/boards/<board_id>/columns", format = "json", data = "<req>")]
pub fn create_column(
    board_id: &str,
    req: Json<CreateColumnRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<ColumnResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    // Require admin role to modify board structure
    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;
    require_not_archived(&conn, board_id)?;

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

// ============ Tasks ============

#[post("/boards/<board_id>/tasks", format = "json", data = "<req>")]
pub fn create_task(
    board_id: &str,
    req: Json<CreateTaskRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    // Require editor role to create tasks
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;

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
            // Verify column belongs to board
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

    // Check WIP limit before creating task in this column
    check_wip_limit(&conn, &column_id, None)?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let creator = key.agent_id.clone().unwrap_or_else(|| key.id.clone());
    let labels_json = serde_json::to_string(&req.labels).unwrap_or_else(|_| "[]".to_string());
    let metadata_json = serde_json::to_string(&req.metadata).unwrap_or_else(|_| "{}".to_string());

    // Determine position: explicit or append to end
    let position: i32 = if let Some(pos) = req.position {
        let pos = pos.max(0);
        // Shift existing tasks to make room
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

    // Log creation event
    let event_data = serde_json::json!({"title": req.title, "task_id": task_id, "column_id": column_id, "creator": creator});
    log_event(&conn, &task_id, "created", &creator, &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.created".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, &task_id)
}

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
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<SearchResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;

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

    // Count total matches (for pagination)
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

    // Order by relevance: title matches first, then by priority
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

#[allow(clippy::too_many_arguments)]
#[get("/boards/<board_id>/tasks?<column>&<assigned>&<claimed>&<priority>&<label>")]
pub fn list_tasks(
    board_id: &str,
    column: Option<&str>,
    assigned: Option<&str>,
    claimed: Option<&str>,
    priority: Option<i32>,
    label: Option<&str>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<TaskResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;

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

#[get("/boards/<board_id>/tasks/<task_id>")]
pub fn get_task(
    board_id: &str,
    task_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;
    load_task_response(&conn, task_id)
}

#[patch("/boards/<board_id>/tasks/<task_id>", format = "json", data = "<req>")]
pub fn update_task(
    board_id: &str,
    task_id: &str,
    req: Json<UpdateTaskRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    // Require editor role to update tasks
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;
    let _existing = load_task_response(&conn, task_id)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());
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
        // Check WIP limit on target column (exclude this task)
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

#[delete("/boards/<board_id>/tasks/<task_id>")]
pub fn delete_task(
    board_id: &str,
    task_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn_check = db.lock().unwrap();
    access::require_role(&conn_check, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn_check, board_id)?;
    drop(conn_check);
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());
    let conn = db.lock().unwrap();
    let affected = conn
        .execute(
            "DELETE FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
        )
        .unwrap_or(0);

    if affected > 0 {
        bus.emit(crate::events::BoardEvent {
            event: "task.deleted".to_string(),
            board_id: board_id.to_string(),
            data: serde_json::json!({"task_id": task_id, "actor": actor}),
        });
        Ok(Json(serde_json::json!({"deleted": true, "id": task_id})))
    } else {
        Err(not_found("Task"))
    }
}

// ============ Agent-First: Claim / Release ============

/// Claim a task — marks you as actively working on it.
/// Different from assignment: assignment is "this is your responsibility",
/// claiming is "I'm working on this right now". Prevents conflicts.
#[post("/boards/<board_id>/tasks/<task_id>/claim")]
pub fn claim_task(
    board_id: &str,
    task_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    // Check if already claimed by someone else
    let current_claim: Option<String> = conn
        .query_row(
            "SELECT claimed_by FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
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
        // Already claimed by us — idempotent, just return
    }

    conn.execute(
        "UPDATE tasks SET claimed_by = ?1, claimed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![actor, task_id],
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

/// Release a claimed task — you're no longer working on it.
#[post("/boards/<board_id>/tasks/<task_id>/release")]
pub fn release_task(
    board_id: &str,
    task_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    conn.execute(
        "UPDATE tasks SET claimed_by = NULL, claimed_at = NULL, updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![task_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let event_data = serde_json::json!({"task_id": task_id, "agent": actor});
    log_event(&conn, task_id, "released", &actor, &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.released".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

/// Move a task to a different column (workflow transition)
#[post("/boards/<board_id>/tasks/<task_id>/move/<target_column_id>")]
pub fn move_task(
    board_id: &str,
    task_id: &str,
    target_column_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    // Require editor role to move tasks
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;

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

    // Check WIP limit on target column (exclude this task since it's being moved)
    check_wip_limit(&conn, target_column_id, Some(task_id))?;

    // Get current column for the event log
    let from_col: String = conn
        .query_row(
            "SELECT column_id FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
            |row| row.get(0),
        )
        .map_err(|_| not_found("Task"))?;

    // Check if moving to a "done" column (last column by position)
    let is_done_column: bool = conn
        .query_row(
            "SELECT position = (SELECT MAX(position) FROM columns WHERE board_id = ?1) FROM columns WHERE id = ?2",
            rusqlite::params![board_id, target_column_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if is_done_column {
        conn.execute(
            "UPDATE tasks SET column_id = ?1, completed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![target_column_id, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    } else {
        conn.execute(
            "UPDATE tasks SET column_id = ?1, completed_at = NULL, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![target_column_id, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    }

    let event_data = serde_json::json!({"task_id": task_id, "from": from_col, "to": target_column_id, "actor": actor});
    log_event(&conn, task_id, "moved", &actor, &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.moved".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

// ============ Task Reorder ============

/// Reorder a task within its column (or move + reorder in one call).
/// Sets the task to the given position and shifts other tasks to make room.
#[post(
    "/boards/<board_id>/tasks/<task_id>/reorder",
    format = "json",
    data = "<req>"
)]
pub fn reorder_task(
    board_id: &str,
    task_id: &str,
    req: Json<ReorderTaskRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    // Get the task's current column
    let current_column: String = conn
        .query_row(
            "SELECT column_id FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![task_id, board_id],
            |row| row.get(0),
        )
        .map_err(|_| not_found("Task"))?;

    let target_column = req.column_id.as_deref().unwrap_or(&current_column);
    let moving_columns = target_column != current_column;

    // If moving to a different column, verify it belongs to the board and check WIP
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

    // If staying in the same column, close the gap where the task was
    if !moving_columns {
        conn.execute(
            "UPDATE tasks SET position = position - 1 WHERE column_id = ?1 AND position > (SELECT position FROM tasks WHERE id = ?2) AND id != ?2",
            rusqlite::params![target_column, task_id],
        )
        .map_err(|e| db_error(&e.to_string()))?;
    }

    // Shift tasks at and after the target position down to make room
    conn.execute(
        "UPDATE tasks SET position = position + 1 WHERE column_id = ?1 AND position >= ?2 AND id != ?3",
        rusqlite::params![target_column, new_pos, task_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    // Place the task at the target position (and column if moving)
    if moving_columns {
        // Check if moving to a "done" column
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

        // Close the gap in the old column
        conn.execute(
            "UPDATE tasks SET position = position - 1 WHERE column_id = ?1 AND position > 0 AND id NOT IN (SELECT id FROM tasks WHERE column_id = ?1 AND position = 0) ORDER BY position",
            rusqlite::params![current_column],
        )
        .ok(); // Best-effort gap cleanup
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
        "actor": actor,
    });
    log_event(&conn, task_id, "reordered", &actor, &event_data);

    bus.emit(crate::events::BoardEvent {
        event: "task.reordered".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_task_response(&conn, task_id)
}

// ============ Batch Operations ============

/// Execute multiple operations on tasks in a single request.
/// Max 50 operations per request. Each operation is independent —
/// failures in one don't roll back others.
#[post("/boards/<board_id>/tasks/batch", format = "json", data = "<req>")]
pub fn batch_tasks(
    board_id: &str,
    req: Json<BatchRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<BatchResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

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
                let result = batch_move(&conn, board_id, task_ids, column_id, &actor, bus);
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
                let result = batch_update(&conn, board_id, task_ids, fields, &actor, bus);
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
                let result = batch_delete(&conn, board_id, task_ids, &actor, bus);
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
    actor: &str,
    bus: &EventBus,
) -> Result<usize, String> {
    // Verify column belongs to board
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

    // Check if target is the "done" column
    let is_done_column: bool = conn
        .query_row(
            "SELECT position = (SELECT MAX(position) FROM columns WHERE board_id = ?1) FROM columns WHERE id = ?2",
            rusqlite::params![board_id, column_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    let mut affected = 0;
    for task_id in task_ids {
        // Verify task belongs to this board
        let belongs: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1 AND board_id = ?2",
                rusqlite::params![task_id, board_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !belongs {
            continue; // Skip tasks not in this board
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
            let event_data = serde_json::json!({"task_id": task_id, "from": from_col, "to": column_id, "actor": actor, "batch": true});
            log_event(conn, task_id, "moved", actor, &event_data);
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
    actor: &str,
    bus: &EventBus,
) -> Result<usize, String> {
    let mut affected = 0;

    for task_id in task_ids {
        // Verify task belongs to this board
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
            log_event(conn, task_id, "updated", actor, &event_data);

            let mut emit_data = changes;
            emit_data.insert("task_id".into(), serde_json::json!(task_id));
            emit_data.insert("actor".into(), serde_json::json!(actor));
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
    actor: &str,
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
                data: serde_json::json!({"task_id": task_id, "actor": actor, "batch": true}),
            });
        }
    }

    Ok(affected)
}

// ============ Task Events ============

#[get("/boards/<board_id>/tasks/<task_id>/events")]
pub fn get_task_events(
    board_id: &str,
    task_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<TaskEventResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;

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

/// Post a comment on a task
#[post(
    "/boards/<board_id>/tasks/<task_id>/comment",
    format = "json",
    data = "<body>"
)]
pub fn comment_on_task(
    board_id: &str,
    task_id: &str,
    body: Json<serde_json::Value>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<TaskEventResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    // Viewers can comment (reading + lightweight contribution)
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

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
    let data = serde_json::json!({"message": message});
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

// ============ Board Collaborators ============

/// List collaborators on a board
#[get("/boards/<board_id>/collaborators")]
pub fn list_collaborators(
    board_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<CollaboratorResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;

    let mut stmt = conn
        .prepare(
            "SELECT bc.key_id, k.name, k.agent_id, bc.role, bc.added_by, bc.created_at
             FROM board_collaborators bc
             JOIN api_keys k ON bc.key_id = k.id
             WHERE bc.board_id = ?1
             ORDER BY bc.created_at ASC",
        )
        .map_err(|e| db_error(&e.to_string()))?;

    let collabs = stmt
        .query_map(rusqlite::params![board_id], |row| {
            Ok(CollaboratorResponse {
                key_id: row.get(0)?,
                key_name: row.get(1)?,
                agent_id: row.get(2)?,
                role: row.get(3)?,
                added_by: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(collabs))
}

/// Add a collaborator to a board (requires admin role on the board)
#[post("/boards/<board_id>/collaborators", format = "json", data = "<req>")]
pub fn add_collaborator(
    board_id: &str,
    req: Json<AddCollaboratorRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<CollaboratorResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    // Require admin role on the board to manage collaborators
    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;

    // Validate role
    let valid_roles = ["viewer", "editor", "admin"];
    if !valid_roles.contains(&req.role.as_str()) {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: format!(
                    "Invalid role '{}'. Valid roles: viewer, editor, admin",
                    req.role
                ),
                code: "INVALID_ROLE".to_string(),
                status: 400,
            }),
        ));
    }

    // Can't add the board owner as a collaborator
    let is_owner: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM boards WHERE id = ?1 AND owner_key_id = ?2",
            rusqlite::params![board_id, req.key_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if is_owner {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Cannot add the board owner as a collaborator".to_string(),
                code: "IS_OWNER".to_string(),
                status: 400,
            }),
        ));
    }

    // Verify the key exists and is active
    let key_info = conn.query_row(
        "SELECT name, agent_id FROM api_keys WHERE id = ?1 AND active = 1",
        rusqlite::params![req.key_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
    );

    let (key_name, agent_id) = match key_info {
        Ok(info) => info,
        Err(_) => {
            return Err((
                Status::NotFound,
                Json(ApiError {
                    error: "API key not found or inactive".to_string(),
                    code: "KEY_NOT_FOUND".to_string(),
                    status: 404,
                }),
            ));
        }
    };

    let adder = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    // Upsert: update role if already a collaborator
    conn.execute(
        "INSERT INTO board_collaborators (board_id, key_id, role, added_by)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(board_id, key_id) DO UPDATE SET role = ?3",
        rusqlite::params![board_id, req.key_id, req.role, adder],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    let created_at: String = conn
        .query_row(
            "SELECT created_at FROM board_collaborators WHERE board_id = ?1 AND key_id = ?2",
            rusqlite::params![board_id, req.key_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());

    Ok(Json(CollaboratorResponse {
        key_id: req.key_id,
        key_name,
        agent_id,
        role: req.role,
        added_by: adder,
        created_at,
    }))
}

/// Remove a collaborator from a board (requires admin role on the board)
#[delete("/boards/<board_id>/collaborators/<collab_key_id>")]
pub fn remove_collaborator(
    board_id: &str,
    collab_key_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;

    let affected = conn
        .execute(
            "DELETE FROM board_collaborators WHERE board_id = ?1 AND key_id = ?2",
            rusqlite::params![board_id, collab_key_id],
        )
        .unwrap_or(0);

    if affected > 0 {
        Ok(Json(
            serde_json::json!({"removed": true, "key_id": collab_key_id}),
        ))
    } else {
        Err((
            Status::NotFound,
            Json(ApiError {
                error: "Collaborator not found on this board".to_string(),
                code: "NOT_FOUND".to_string(),
                status: 404,
            }),
        ))
    }
}

// ============ API Keys ============

#[get("/keys")]
pub fn list_keys(
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<KeyResponse>>, (Status, Json<ApiError>)> {
    if !key.is_admin {
        return Err(forbidden());
    }

    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id, name, agent_id, created_at, last_used_at, requests_count, rate_limit, active
             FROM api_keys ORDER BY created_at DESC",
        )
        .map_err(|e| db_error(&e.to_string()))?;

    let keys = stmt
        .query_map([], |row| {
            Ok(KeyResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                agent_id: row.get(2)?,
                key: None,
                created_at: row.get(3)?,
                last_used_at: row.get(4)?,
                requests_count: row.get(5)?,
                rate_limit: row.get(6)?,
                active: row.get::<_, i32>(7)? == 1,
            })
        })
        .map_err(|e| db_error(&e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(keys))
}

#[post("/keys", format = "json", data = "<req>")]
pub fn create_key(
    req: Json<CreateKeyRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<KeyResponse>, (Status, Json<ApiError>)> {
    if !key.is_admin {
        return Err(forbidden());
    }

    let req = req.into_inner();
    let new_key = format!("kb_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let key_hash = crate::db::hash_key(&new_key);
    let id = uuid::Uuid::new_v4().to_string();

    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO api_keys (id, name, key_hash, agent_id, rate_limit) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, req.name, key_hash, req.agent_id, req.rate_limit],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    Ok(Json(KeyResponse {
        id,
        name: req.name,
        agent_id: req.agent_id,
        key: Some(new_key),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_used_at: None,
        requests_count: 0,
        rate_limit: req.rate_limit,
        active: true,
    }))
}

#[delete("/keys/<id>")]
pub fn delete_key(
    id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    if !key.is_admin {
        return Err(forbidden());
    }

    let conn = db.lock().unwrap();
    let affected = conn
        .execute(
            "UPDATE api_keys SET active = 0 WHERE id = ?1",
            rusqlite::params![id],
        )
        .unwrap_or(0);

    if affected > 0 {
        Ok(Json(serde_json::json!({"revoked": true, "id": id})))
    } else {
        Err(not_found("API key"))
    }
}

// ============ Webhooks ============

/// Register a webhook for a board. Requires Admin role.
/// Returns the webhook with its secret (shown only once).
#[post("/boards/<board_id>/webhooks", format = "json", data = "<req>")]
pub fn create_webhook(
    board_id: &str,
    req: Json<CreateWebhookRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<WebhookResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;

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

    // Validate event types if provided
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
    let creator = key.agent_id.clone().unwrap_or_else(|| key.id.clone());
    let events_json = serde_json::to_string(&req.events).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO webhooks (id, board_id, url, secret, events, created_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![webhook_id, board_id, req.url.trim(), secret, events_json, creator],
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

/// List webhooks for a board. Requires Admin role.
/// Secrets are never shown after creation.
#[get("/boards/<board_id>/webhooks")]
pub fn list_webhooks(
    board_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<WebhookResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;

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

/// Update a webhook (URL, events filter, active status). Requires Admin role.
#[patch(
    "/boards/<board_id>/webhooks/<webhook_id>",
    format = "json",
    data = "<req>"
)]
pub fn update_webhook(
    board_id: &str,
    webhook_id: &str,
    req: Json<UpdateWebhookRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<WebhookResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;

    // Verify webhook exists and belongs to this board
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
        // Reset failure count when re-enabling
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

    // Load and return updated webhook
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

/// Delete a webhook. Requires Admin role.
#[delete("/boards/<board_id>/webhooks/<webhook_id>")]
pub fn delete_webhook(
    board_id: &str,
    webhook_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Admin)?;

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

/// Add a dependency between two tasks on the same board.
/// "blocker blocks blocked" — blocked_task cannot proceed until blocker_task is complete.
#[post("/boards/<board_id>/dependencies", format = "json", data = "<req>")]
pub fn create_dependency(
    board_id: &str,
    req: Json<CreateDependencyRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<DependencyResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    // Can't depend on yourself
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

    // Both tasks must belong to this board
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

    // Check for circular dependency (would the reverse already exist?)
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

    // Check for transitive circular deps: can we reach blocker from blocked via existing deps?
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
        "INSERT INTO task_dependencies (id, board_id, blocker_task_id, blocked_task_id, created_by, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![dep_id, board_id, req.blocker_task_id, req.blocked_task_id, actor, req.note],
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

    // Log event on the blocked task
    let event_data = serde_json::json!({
        "dependency_id": dep_id,
        "blocker_task_id": req.blocker_task_id,
        "blocked_task_id": req.blocked_task_id,
        "note": req.note,
        "actor": actor,
    });
    log_event(
        &conn,
        &req.blocked_task_id,
        "dependency.added",
        &actor,
        &event_data,
    );

    bus.emit(crate::events::BoardEvent {
        event: "task.dependency.added".to_string(),
        board_id: board_id.to_string(),
        data: event_data,
    });

    load_dependency_response(&conn, &dep_id)
}

/// List all dependencies for a board, or filter by a specific task.
/// Use `?task=<id>` to see all dependencies involving that task (as blocker or blocked).
#[get("/boards/<board_id>/dependencies?<task>")]
pub fn list_dependencies(
    board_id: &str,
    task: Option<&str>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<DependencyResponse>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Viewer)?;

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

/// Delete a dependency.
#[delete("/boards/<board_id>/dependencies/<dep_id>")]
pub fn delete_dependency(
    board_id: &str,
    dep_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
    bus: &State<EventBus>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    require_not_archived(&conn, board_id)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    // Get dep info before deleting
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
            "actor": actor,
        });
        log_event(
            &conn,
            &blocked_id,
            "dependency.removed",
            &actor,
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

/// Check if there's a path from `from_task` to `to_task` via blocker_task_id → blocked_task_id edges.
/// Used for transitive circular dependency detection.
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
        // Follow edges: current blocks X → X is in blocked_task_id where current is blocker
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
            "SELECT b.id, b.name, b.description, b.archived, b.created_at, b.updated_at,
                    COALESCE(k.agent_id, b.owner_key_id)
             FROM boards b
             LEFT JOIN api_keys k ON b.owner_key_id = k.id
             WHERE b.id = ?1",
            rusqlite::params![board_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)? == 1,
                    row.get::<_, String>(4)?,
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
        owner: board.6,
        columns,
        task_count,
        archived: board.3,
        created_at: board.4,
        updated_at: board.5,
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

fn forbidden() -> (Status, Json<ApiError>) {
    (
        Status::Forbidden,
        Json(ApiError {
            error: "Admin access required".to_string(),
            code: "FORBIDDEN".to_string(),
            status: 403,
        }),
    )
}

/// Check if a board is archived. Returns error if it is.
fn require_not_archived(conn: &Connection, board_id: &str) -> Result<(), (Status, Json<ApiError>)> {
    let archived: bool = conn
        .query_row(
            "SELECT archived = 1 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if archived {
        return Err((
            Status::Conflict,
            Json(ApiError {
                error: "Board is archived. Unarchive it before making changes.".to_string(),
                code: "BOARD_ARCHIVED".to_string(),
                status: 409,
            }),
        ));
    }

    Ok(())
}

/// Check if adding a task to a column would exceed its WIP limit.
/// `exclude_task_id` allows excluding a specific task (for moves — the task being moved
/// is already counted in its current column, not the target).
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

/// Catch-all route for client-side routing. Serves index.html for any GET
/// request that didn't match an API route or static file.
/// Rank 20 ensures this runs after FileServer and all other routes.
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
