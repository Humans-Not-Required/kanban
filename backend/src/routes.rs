use rocket::http::{ContentType, Status};
use rocket::serde::json::Json;
use rocket::State;

use crate::access::{self, BoardRole};
use crate::auth::AuthenticatedKey;
use crate::db::DbPool;
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

#[get("/boards")]
pub fn list_boards(
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<BoardSummary>>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();

    // Admin keys see all boards; regular keys see boards they own, collaborate on, or have tasks in
    let (sql, param1, param2) = if key.is_admin {
        (
            "SELECT b.id, b.name, b.description, b.archived, b.created_at,
                (SELECT COUNT(*) FROM tasks t WHERE t.board_id = b.id)
             FROM boards b
             ORDER BY b.created_at DESC"
                .to_string(),
            String::new(),
            String::new(),
        )
    } else {
        let agent = key.agent_id.as_deref().unwrap_or(&key.id);
        (
            "SELECT b.id, b.name, b.description, b.archived, b.created_at,
                (SELECT COUNT(*) FROM tasks t WHERE t.board_id = b.id)
             FROM boards b
             WHERE b.owner_key_id = ?1
                OR b.id IN (SELECT board_id FROM board_collaborators WHERE key_id = ?1)
                OR b.id IN (SELECT DISTINCT board_id FROM tasks WHERE created_by = ?1 OR assigned_to = ?2 OR claimed_by = ?2)
             ORDER BY b.created_at DESC".to_string(),
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
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    // Require editor role to create tasks
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;

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

    // Get next position in column
    let position: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE column_id = ?1",
            rusqlite::params![column_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

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
    log_event(
        &conn,
        &task_id,
        "created",
        &creator,
        &serde_json::json!({"title": req.title}),
    );

    load_task_response(&conn, &task_id)
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
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();
    let conn = db.lock().unwrap();

    // Require editor role to update tasks
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
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
        log_event(
            &conn,
            task_id,
            "updated",
            &actor,
            &serde_json::Value::Object(changes),
        );
    }

    load_task_response(&conn, task_id)
}

#[delete("/boards/<board_id>/tasks/<task_id>")]
pub fn delete_task(
    board_id: &str,
    task_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn_check = db.lock().unwrap();
    access::require_role(&conn_check, board_id, &key, BoardRole::Editor)?;
    drop(conn_check);
    let conn = db.lock().unwrap();
    let affected = conn
        .execute(
            "DELETE FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
        )
        .unwrap_or(0);

    if affected > 0 {
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
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
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

    log_event(
        &conn,
        task_id,
        "claimed",
        &actor,
        &serde_json::json!({"agent": actor}),
    );

    load_task_response(&conn, task_id)
}

/// Release a claimed task — you're no longer working on it.
#[post("/boards/<board_id>/tasks/<task_id>/release")]
pub fn release_task(
    board_id: &str,
    task_id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    conn.execute(
        "UPDATE tasks SET claimed_by = NULL, claimed_at = NULL, updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![task_id],
    )
    .map_err(|e| db_error(&e.to_string()))?;

    log_event(
        &conn,
        task_id,
        "released",
        &actor,
        &serde_json::json!({"agent": actor}),
    );

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
) -> Result<Json<TaskResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let actor = key.agent_id.clone().unwrap_or_else(|| key.id.clone());

    // Require editor role to move tasks
    access::require_role(&conn, board_id, &key, BoardRole::Editor)?;

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

    log_event(
        &conn,
        task_id,
        "moved",
        &actor,
        &serde_json::json!({"from": from_col, "to": target_column_id}),
    );

    load_task_response(&conn, task_id)
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

// ============ Helpers ============

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
