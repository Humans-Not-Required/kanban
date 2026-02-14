// Integration tests for kanban service — per-board token auth model

#[test]
fn test_db_initialization() {
    let db_path = format!("/tmp/kanban_test_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    // Verify tables exist
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"boards".to_string()));
    assert!(tables.contains(&"columns".to_string()));
    assert!(tables.contains(&"tasks".to_string()));
    assert!(tables.contains(&"task_events".to_string()));
    assert!(tables.contains(&"webhooks".to_string()));
    assert!(tables.contains(&"task_dependencies".to_string()));

    // Verify boards table has manage_key_hash column
    let col_names: Vec<String> = conn
        .prepare("PRAGMA table_info(boards)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(col_names.contains(&"manage_key_hash".to_string()));
    assert!(col_names.contains(&"is_public".to_string()));

    // No api_keys table in new schema
    assert!(!tables.contains(&"api_keys".to_string()));
    // No board_collaborators table in new schema
    assert!(!tables.contains(&"board_collaborators".to_string()));

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_key_hashing() {
    let hash1 = kanban::db::hash_key("test_key_123");
    let hash2 = kanban::db::hash_key("test_key_123");
    let hash3 = kanban::db::hash_key("different_key");

    assert_eq!(hash1, hash2, "Same input should produce same hash");
    assert_ne!(hash1, hash3, "Different input should produce different hash");
    assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()), "Hash should be hex");
}

#[test]
fn test_db_wal_mode() {
    let db_path = format!("/tmp/kanban_test_wal_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode, "wal");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(format!("{}-wal", db_path));
    let _ = std::fs::remove_file(format!("{}-shm", db_path));
}

#[test]
fn test_board_creation_and_manage_key() {
    let db_path = format!("/tmp/kanban_test_board_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    // Create a board with a manage key
    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key = format!("kb_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let manage_key_hash = kanban::db::hash_key(&manage_key);

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash, is_public) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![board_id, "Test Board", "A test board", manage_key_hash, 0],
    )
    .unwrap();

    // Verify board exists
    let name: String = conn
        .query_row(
            "SELECT name FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(name, "Test Board");

    // Verify manage key hash matches
    let stored_hash: String = conn
        .query_row(
            "SELECT manage_key_hash FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_hash, manage_key_hash);

    // Verify a wrong key doesn't match
    let wrong_hash = kanban::db::hash_key("wrong_key");
    assert_ne!(stored_hash, wrong_hash);

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_access_control_manage_key() {
    use kanban::access;

    let db_path = format!("/tmp/kanban_test_access_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    // Create a board
    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key = "kb_test_manage_key_12345";
    let manage_key_hash = kanban::db::hash_key(manage_key);

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![board_id, "Access Test", "", manage_key_hash],
    )
    .unwrap();

    // Correct manage key should pass
    let result = access::require_manage_key(&conn, &board_id, &manage_key_hash);
    assert!(result.is_ok(), "Correct manage key should succeed");

    // Wrong key should fail
    let wrong_hash = kanban::db::hash_key("wrong_key");
    let result = access::require_manage_key(&conn, &board_id, &wrong_hash);
    assert!(result.is_err(), "Wrong manage key should fail");

    // Nonexistent board should fail
    let result = access::require_manage_key(&conn, "nonexistent-id", &manage_key_hash);
    assert!(result.is_err(), "Nonexistent board should fail");

    // Board exists check
    let result = access::require_board_exists(&conn, &board_id);
    assert!(result.is_ok(), "Existing board should pass");

    let result = access::require_board_exists(&conn, "nonexistent-id");
    assert!(result.is_err(), "Nonexistent board should fail");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_board_archiving() {
    use kanban::access;

    let db_path = format!("/tmp/kanban_test_archive_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key_hash = kanban::db::hash_key("test_key");

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash) VALUES (?1, ?2, '', ?3)",
        rusqlite::params![board_id, "Archive Test", manage_key_hash],
    )
    .unwrap();

    // Create column + task
    let col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Backlog', 0)",
        rusqlite::params![col_id, board_id],
    )
    .unwrap();

    let task_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tasks (id, board_id, column_id, title, created_by) VALUES (?1, ?2, ?3, 'Test Task', 'admin')",
        rusqlite::params![task_id, board_id, col_id],
    )
    .unwrap();

    // Not archived initially
    let result = access::require_not_archived(&conn, &board_id);
    assert!(result.is_ok(), "Board should not be archived initially");

    // Archive the board
    conn.execute(
        "UPDATE boards SET archived = 1, updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![board_id],
    )
    .unwrap();

    // Now require_not_archived should fail
    let result = access::require_not_archived(&conn, &board_id);
    assert!(result.is_err(), "Archived board should fail require_not_archived");

    // Unarchive
    conn.execute(
        "UPDATE boards SET archived = 0, updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![board_id],
    )
    .unwrap();

    let result = access::require_not_archived(&conn, &board_id);
    assert!(result.is_ok(), "Unarchived board should pass");

    // Task still exists
    let task_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(task_exists, "Tasks preserved through archive/unarchive cycle");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_wip_limit_enforcement() {
    let db_path = format!("/tmp/kanban_test_wip_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key_hash = kanban::db::hash_key("test_key");

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash) VALUES (?1, 'WIP Test', '', ?2)",
        rusqlite::params![board_id, manage_key_hash],
    )
    .unwrap();

    // Column with WIP limit of 2
    let limited_col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position, wip_limit) VALUES (?1, ?2, 'Limited', 0, 2)",
        rusqlite::params![limited_col_id, board_id],
    )
    .unwrap();

    // Unlimited column
    let unlimited_col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Unlimited', 1)",
        rusqlite::params![unlimited_col_id, board_id],
    )
    .unwrap();

    // Add 2 tasks to limited column
    for i in 0..2 {
        let tid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tasks (id, board_id, column_id, title, position, created_by) VALUES (?1, ?2, ?3, ?4, ?5, 'test')",
            rusqlite::params![tid, board_id, limited_col_id, format!("Task {}", i), i],
        )
        .unwrap();
    }

    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE column_id = ?1",
            rusqlite::params![limited_col_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);

    // WIP limit stored correctly
    let wip: Option<i32> = conn
        .query_row(
            "SELECT wip_limit FROM columns WHERE id = ?1",
            rusqlite::params![limited_col_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(wip, Some(2));

    // Unlimited column has no limit
    let no_wip: Option<i32> = conn
        .query_row(
            "SELECT wip_limit FROM columns WHERE id = ?1",
            rusqlite::params![unlimited_col_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(no_wip, None);

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_task_search() {
    let db_path = format!("/tmp/kanban_test_search_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key_hash = kanban::db::hash_key("test_key");

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash) VALUES (?1, 'Search Test', '', ?2)",
        rusqlite::params![board_id, manage_key_hash],
    )
    .unwrap();

    let col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Todo', 0)",
        rusqlite::params![col_id, board_id],
    )
    .unwrap();

    let tasks = [
        ("Fix login bug", "Users cannot login with OAuth", r#"["bug","auth"]"#),
        ("Add search endpoint", "Implement full-text search for tasks", r#"["feature","api"]"#),
        ("Update auth docs", "Document the OAuth flow changes", r#"["docs","auth"]"#),
        ("Deploy to production", "Final deployment steps", r#"["ops"]"#),
    ];

    for (i, (title, desc, labels)) in tasks.iter().enumerate() {
        let tid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tasks (id, board_id, column_id, title, description, labels, position, created_by, priority)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'test', ?8)",
            rusqlite::params![tid, board_id, col_id, title, desc, labels, i as i32, (4 - i) as i32],
        )
        .unwrap();
    }

    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*) FROM tasks WHERE board_id = ?1 AND (title LIKE ?2 OR description LIKE ?2 OR labels LIKE ?2)",
        )
        .unwrap();

    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%login%"], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1, "Should find 1 task matching 'login'");

    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%OAuth%"], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2, "Should find 2 tasks matching 'OAuth'");

    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%auth%"], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2, "Should find 2 tasks matching 'auth'");

    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%xyznonexistent%"], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0, "Should find 0 tasks for nonsense query");

    drop(stmt);
    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_task_ordering_positions() {
    let db_path = format!("/tmp/kanban_test_order_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key_hash = kanban::db::hash_key("test_key");

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash) VALUES (?1, 'Order Test', '', ?2)",
        rusqlite::params![board_id, manage_key_hash],
    )
    .unwrap();

    let col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Todo', 0)",
        rusqlite::params![col_id, board_id],
    )
    .unwrap();

    let mut task_ids = Vec::new();
    for i in 0..4 {
        let tid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tasks (id, board_id, column_id, title, position, created_by) VALUES (?1, ?2, ?3, ?4, ?5, 'test')",
            rusqlite::params![tid, board_id, col_id, format!("Task {}", i), i],
        )
        .unwrap();
        task_ids.push(tid);
    }

    // Simulate reorder: move Task 3 to position 1
    conn.execute(
        "UPDATE tasks SET position = position - 1 WHERE column_id = ?1 AND position > 3 AND id != ?2",
        rusqlite::params![col_id, task_ids[3]],
    )
    .unwrap();

    conn.execute(
        "UPDATE tasks SET position = position + 1 WHERE column_id = ?1 AND position >= 1 AND id != ?2",
        rusqlite::params![col_id, task_ids[3]],
    )
    .unwrap();

    conn.execute(
        "UPDATE tasks SET position = 1 WHERE id = ?1",
        rusqlite::params![task_ids[3]],
    )
    .unwrap();

    let positions: Vec<(String, i32)> = conn
        .prepare("SELECT title, position FROM tasks WHERE column_id = ?1 ORDER BY position")
        .unwrap()
        .query_map(rusqlite::params![col_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(positions.len(), 4);
    assert_eq!(positions[0].0, "Task 0");
    assert_eq!(positions[1].0, "Task 3");
    assert_eq!(positions[2].0, "Task 1");
    assert_eq!(positions[3].0, "Task 2");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_batch_operations() {
    let db_path = format!("/tmp/kanban_test_batch_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key_hash = kanban::db::hash_key("test_key");

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash) VALUES (?1, 'Batch Board', '', ?2)",
        rusqlite::params![board_id, manage_key_hash],
    )
    .unwrap();

    let col_backlog = uuid::Uuid::new_v4().to_string();
    let col_done = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Backlog', 0)",
        rusqlite::params![col_backlog, board_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Done', 1)",
        rusqlite::params![col_done, board_id],
    )
    .unwrap();

    let mut task_ids = Vec::new();
    for i in 0..5 {
        let task_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tasks (id, board_id, column_id, title, priority, position, created_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'test')",
            rusqlite::params![task_id, board_id, col_backlog, format!("Batch Task {}", i), i, i],
        )
        .unwrap();
        task_ids.push(task_id);
    }

    // Move first 3 tasks to Done
    for tid in &task_ids[0..3] {
        conn.execute(
            "UPDATE tasks SET column_id = ?1, completed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![col_done, tid],
        )
        .unwrap();
    }

    let done_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE board_id = ?1 AND column_id = ?2",
            rusqlite::params![board_id, col_done],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(done_count, 3);

    // Batch update priority
    for tid in &task_ids[3..5] {
        conn.execute(
            "UPDATE tasks SET priority = 99, updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![tid],
        )
        .unwrap();
    }

    let priority: i32 = conn
        .query_row(
            "SELECT priority FROM tasks WHERE id = ?1",
            rusqlite::params![task_ids[3]],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(priority, 99);

    // Batch delete done tasks
    for tid in &task_ids[0..3] {
        conn.execute(
            "DELETE FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![tid, board_id],
        )
        .unwrap();
    }

    let total: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE board_id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(total, 2);

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_webhooks_crud() {
    let db_path = format!("/tmp/kanban_test_webhooks_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key_hash = kanban::db::hash_key("test_key");

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash) VALUES (?1, 'Webhook Board', '', ?2)",
        rusqlite::params![board_id, manage_key_hash],
    )
    .unwrap();

    // Create webhook
    let webhook_id = uuid::Uuid::new_v4().to_string();
    let events_json = serde_json::to_string(&vec!["task.created", "task.moved"]).unwrap();
    conn.execute(
        "INSERT INTO webhooks (id, board_id, url, secret, events) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![webhook_id, board_id, "https://example.com/webhook", "whsec_test123", events_json],
    )
    .unwrap();

    let (url, active, failure_count): (String, i32, i32) = conn
        .query_row(
            "SELECT url, active, failure_count FROM webhooks WHERE id = ?1",
            rusqlite::params![webhook_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(url, "https://example.com/webhook");
    assert_eq!(active, 1);
    assert_eq!(failure_count, 0);

    // Update URL
    conn.execute(
        "UPDATE webhooks SET url = ?1 WHERE id = ?2",
        rusqlite::params!["https://example.com/webhook/v2", webhook_id],
    )
    .unwrap();

    // Deactivate
    conn.execute(
        "UPDATE webhooks SET active = 0 WHERE id = ?1",
        rusqlite::params![webhook_id],
    )
    .unwrap();

    // Delete
    let affected = conn
        .execute("DELETE FROM webhooks WHERE id = ?1", rusqlite::params![webhook_id])
        .unwrap();
    assert_eq!(affected, 1);

    // Cascade on board delete
    let wh_id2 = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO webhooks (id, board_id, url, secret, events) VALUES (?1, ?2, ?3, ?4, '[]')",
        rusqlite::params![wh_id2, board_id, "https://example.com/wh2", "secret2"],
    )
    .unwrap();

    conn.execute("DELETE FROM boards WHERE id = ?1", rusqlite::params![board_id])
        .unwrap();

    let orphan_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM webhooks WHERE board_id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(orphan_count, 0, "Webhooks cascade-deleted with board");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_task_dependencies() {
    let db_path = format!("/tmp/kanban_test_deps_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let board_id = uuid::Uuid::new_v4().to_string();
    let manage_key_hash = kanban::db::hash_key("test_key");

    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash) VALUES (?1, 'Deps Test', '', ?2)",
        rusqlite::params![board_id, manage_key_hash],
    )
    .unwrap();

    let col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Backlog', 0)",
        rusqlite::params![col_id, board_id],
    )
    .unwrap();

    let task_a = uuid::Uuid::new_v4().to_string();
    let task_b = uuid::Uuid::new_v4().to_string();
    let task_c = uuid::Uuid::new_v4().to_string();

    for (id, title, pos) in [(&task_a, "Task A", 0), (&task_b, "Task B", 1), (&task_c, "Task C", 2)] {
        conn.execute(
            "INSERT INTO tasks (id, board_id, column_id, title, position, created_by) VALUES (?1, ?2, ?3, ?4, ?5, 'admin')",
            rusqlite::params![id, board_id, col_id, title, pos],
        )
        .unwrap();
    }

    // A blocks B
    let dep_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO task_dependencies (id, board_id, blocker_task_id, blocked_task_id, note) VALUES (?1, ?2, ?3, ?4, 'A must finish first')",
        rusqlite::params![dep_id, board_id, task_a, task_b],
    )
    .unwrap();

    let dep_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_dependencies WHERE board_id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(dep_count, 1);

    // UNIQUE constraint
    let dup_result = conn.execute(
        "INSERT INTO task_dependencies (id, board_id, blocker_task_id, blocked_task_id) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), board_id, task_a, task_b],
    );
    assert!(dup_result.is_err(), "Duplicate dependency should fail");

    // B blocks C (chain: A → B → C)
    conn.execute(
        "INSERT INTO task_dependencies (id, board_id, blocker_task_id, blocked_task_id) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), board_id, task_b, task_c],
    )
    .unwrap();

    // Delete task B → cascade removes its dependencies
    conn.execute("DELETE FROM tasks WHERE id = ?1", rusqlite::params![task_b])
        .unwrap();

    let after_cascade: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_dependencies WHERE board_id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(after_cascade, 0, "Dependencies cascade-deleted with task");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_board_public_listing() {
    let db_path = format!("/tmp/kanban_test_public_{}.db", uuid::Uuid::new_v4());
    let pool = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let manage_key_hash = kanban::db::hash_key("test_key");

    // Create a public board
    let public_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash, is_public) VALUES (?1, 'Public Board', '', ?2, 1)",
        rusqlite::params![public_id, manage_key_hash],
    )
    .unwrap();

    // Create an unlisted board
    let unlisted_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO boards (id, name, description, manage_key_hash, is_public) VALUES (?1, 'Unlisted Board', '', ?2, 0)",
        rusqlite::params![unlisted_id, manage_key_hash],
    )
    .unwrap();

    // Only public boards show in listing
    let public_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM boards WHERE is_public = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(public_count, 1);

    // But both are accessible by UUID
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .unwrap();
    assert_eq!(total, 2);

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_rate_limiter() {
    use kanban::rate_limit::RateLimiter;
    use std::time::Duration;

    let rl = RateLimiter::new(Duration::from_secs(60), 3);
    let key_id = "test-rate-limit-key";
    let limit = 3u64;

    for i in 0..3 {
        let result = rl.check(key_id, limit);
        assert!(result.allowed, "Request {} should be allowed", i + 1);
        assert_eq!(result.remaining, 2 - i);
    }

    let result = rl.check(key_id, limit);
    assert!(!result.allowed, "4th request should be blocked");
    assert_eq!(result.remaining, 0);

    let result = rl.check("other-key", limit);
    assert!(result.allowed, "Different key unaffected");
}
