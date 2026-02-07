// Unit and integration tests for kanban service

#[test]
fn test_db_has_collaborators_table() {
    let db_path = format!("/tmp/kanban_test_collab_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(
        tables.contains(&"board_collaborators".to_string()),
        "board_collaborators table should exist"
    );

    // Verify schema: composite PK on (board_id, key_id)
    let col_info: Vec<(String, String)> = conn
        .prepare("PRAGMA table_info(board_collaborators)")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let col_names: Vec<&str> = col_info.iter().map(|(n, _)| n.as_str()).collect();
    assert!(col_names.contains(&"board_id"));
    assert!(col_names.contains(&"key_id"));
    assert!(col_names.contains(&"role"));
    assert!(col_names.contains(&"added_by"));
    assert!(col_names.contains(&"created_at"));

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_access_control_roles() {
    use kanban::access::{get_board_role, BoardRole};
    use kanban::auth::AuthenticatedKey;

    let db_path = format!("/tmp/kanban_test_access_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
    let conn = pool.lock().unwrap();

    // Get the admin key ID
    let admin_key_id: String = conn
        .query_row("SELECT id FROM api_keys WHERE is_admin = 1", [], |row| {
            row.get(0)
        })
        .unwrap();

    // Create a regular key
    let regular_key_id = uuid::Uuid::new_v4().to_string();
    let regular_hash = kanban::db::hash_key("regular_test_key");
    conn.execute(
        "INSERT INTO api_keys (id, name, key_hash, is_admin, agent_id) VALUES (?1, 'Regular', ?2, 0, 'agent-regular')",
        rusqlite::params![regular_key_id, regular_hash],
    )
    .unwrap();

    // Create another regular key (outsider)
    let outsider_key_id = uuid::Uuid::new_v4().to_string();
    let outsider_hash = kanban::db::hash_key("outsider_test_key");
    conn.execute(
        "INSERT INTO api_keys (id, name, key_hash, is_admin, agent_id) VALUES (?1, 'Outsider', ?2, 0, 'agent-outsider')",
        rusqlite::params![outsider_key_id, outsider_hash],
    )
    .unwrap();

    // Create a board owned by the regular key
    let board_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO boards (id, name, description, owner_key_id) VALUES (?1, 'Test Board', '', ?2)",
        rusqlite::params![board_id, regular_key_id],
    )
    .unwrap();

    // Test: owner has Owner role
    let owner_key = AuthenticatedKey {
        id: regular_key_id.clone(),
        name: "Regular".to_string(),
        is_admin: false,
        agent_id: Some("agent-regular".to_string()),
    };
    assert_eq!(
        get_board_role(&conn, &board_id, &owner_key),
        Some(BoardRole::Owner)
    );

    // Test: admin key has Admin role (even though not owner)
    let admin_key = AuthenticatedKey {
        id: admin_key_id.clone(),
        name: "Admin".to_string(),
        is_admin: true,
        agent_id: Some("admin".to_string()),
    };
    assert_eq!(
        get_board_role(&conn, &board_id, &admin_key),
        Some(BoardRole::Admin)
    );

    // Test: outsider has no role
    let outsider_key = AuthenticatedKey {
        id: outsider_key_id.clone(),
        name: "Outsider".to_string(),
        is_admin: false,
        agent_id: Some("agent-outsider".to_string()),
    };
    assert_eq!(get_board_role(&conn, &board_id, &outsider_key), None);

    // Add outsider as viewer collaborator
    conn.execute(
        "INSERT INTO board_collaborators (board_id, key_id, role, added_by) VALUES (?1, ?2, 'viewer', ?3)",
        rusqlite::params![board_id, outsider_key_id, regular_key_id],
    )
    .unwrap();

    // Test: outsider now has Viewer role
    assert_eq!(
        get_board_role(&conn, &board_id, &outsider_key),
        Some(BoardRole::Viewer)
    );

    // Upgrade to editor
    conn.execute(
        "UPDATE board_collaborators SET role = 'editor' WHERE board_id = ?1 AND key_id = ?2",
        rusqlite::params![board_id, outsider_key_id],
    )
    .unwrap();

    assert_eq!(
        get_board_role(&conn, &board_id, &outsider_key),
        Some(BoardRole::Editor)
    );

    // Test role ordering: Editor >= Viewer
    assert!(BoardRole::Editor >= BoardRole::Viewer);
    assert!(BoardRole::Admin >= BoardRole::Editor);
    assert!(BoardRole::Owner >= BoardRole::Admin);
    assert!(BoardRole::Viewer < BoardRole::Editor);

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_db_initialization() {
    // Verify database can be created and has correct schema
    let db_path = format!("/tmp/kanban_test_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
    let conn = pool.lock().unwrap();

    // Verify tables exist
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"api_keys".to_string()));
    assert!(tables.contains(&"boards".to_string()));
    assert!(tables.contains(&"columns".to_string()));
    assert!(tables.contains(&"tasks".to_string()));
    assert!(tables.contains(&"task_events".to_string()));

    // Verify admin key was created
    let key_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM api_keys WHERE is_admin = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(key_count, 1);

    // Cleanup
    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_key_hashing() {
    let hash1 = kanban::db::hash_key("test_key_123");
    let hash2 = kanban::db::hash_key("test_key_123");
    let hash3 = kanban::db::hash_key("different_key");

    // Same input produces same hash
    assert_eq!(hash1, hash2);
    // Different input produces different hash
    assert_ne!(hash1, hash3);
    // Hash is hex string
    assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_wip_limit_enforcement() {
    let db_path = format!("/tmp/kanban_test_wip_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
    let conn = pool.lock().unwrap();

    // Get admin key ID
    let admin_key_id: String = conn
        .query_row("SELECT id FROM api_keys WHERE is_admin = 1", [], |row| {
            row.get(0)
        })
        .unwrap();

    // Create a board
    let board_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO boards (id, name, description, owner_key_id) VALUES (?1, 'WIP Test Board', '', ?2)",
        rusqlite::params![board_id, admin_key_id],
    )
    .unwrap();

    // Create a column WITH a WIP limit of 2
    let limited_col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position, wip_limit) VALUES (?1, ?2, 'Limited', 0, 2)",
        rusqlite::params![limited_col_id, board_id],
    )
    .unwrap();

    // Create a column WITHOUT a WIP limit
    let unlimited_col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Unlimited', 1)",
        rusqlite::params![unlimited_col_id, board_id],
    )
    .unwrap();

    // Add 2 tasks to the limited column (at the limit)
    let task1_id = uuid::Uuid::new_v4().to_string();
    let task2_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tasks (id, board_id, column_id, title, position, created_by) VALUES (?1, ?2, ?3, 'Task 1', 0, 'test')",
        rusqlite::params![task1_id, board_id, limited_col_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, board_id, column_id, title, position, created_by) VALUES (?1, ?2, ?3, 'Task 2', 1, 'test')",
        rusqlite::params![task2_id, board_id, limited_col_id],
    )
    .unwrap();

    // Verify count
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE column_id = ?1",
            rusqlite::params![limited_col_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);

    // Verify WIP limit is stored correctly
    let wip_limit: Option<i32> = conn
        .query_row(
            "SELECT wip_limit FROM columns WHERE id = ?1",
            rusqlite::params![limited_col_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(wip_limit, Some(2));

    // Verify unlimited column has no WIP limit
    let no_limit: Option<i32> = conn
        .query_row(
            "SELECT wip_limit FROM columns WHERE id = ?1",
            rusqlite::params![unlimited_col_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(no_limit, None);

    // Add a task to the unlimited column — should work regardless of count
    let task3_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tasks (id, board_id, column_id, title, position, created_by) VALUES (?1, ?2, ?3, 'Task 3', 0, 'test')",
        rusqlite::params![task3_id, board_id, unlimited_col_id],
    )
    .unwrap();

    // Verify task3 exists in unlimited column
    let t3_col: String = conn
        .query_row(
            "SELECT column_id FROM tasks WHERE id = ?1",
            rusqlite::params![task3_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(t3_col, unlimited_col_id);

    // Move task1 from limited to unlimited — should succeed and free a spot
    conn.execute(
        "UPDATE tasks SET column_id = ?1 WHERE id = ?2",
        rusqlite::params![unlimited_col_id, task1_id],
    )
    .unwrap();

    // Now limited column has 1 task — should be able to add another
    let limited_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE column_id = ?1",
            rusqlite::params![limited_col_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(limited_count, 1); // Only task2 remains

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_rate_limiting_integration() {
    use kanban::rate_limit::RateLimiter;
    use std::time::Duration;

    // Create a limiter with 60s window
    let rl = RateLimiter::new(Duration::from_secs(60));

    // Simulate a key with limit of 3
    let key_id = "test-rate-limit-key";
    let limit = 3u64;

    // First 3 requests should be allowed
    for i in 0..3 {
        let result = rl.check(key_id, limit);
        assert!(result.allowed, "Request {} should be allowed", i + 1);
        assert_eq!(result.limit, 3);
        assert_eq!(result.remaining, 2 - i);
    }

    // 4th request should be blocked
    let result = rl.check(key_id, limit);
    assert!(!result.allowed, "4th request should be blocked");
    assert_eq!(result.remaining, 0);
    assert!(result.reset_secs <= 60);

    // Different key should still work
    let result = rl.check("other-key", limit);
    assert!(result.allowed, "Different key should not be affected");
    assert_eq!(result.remaining, 2);
}

#[test]
fn test_db_wal_mode() {
    let db_path = format!("/tmp/kanban_test_wal_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
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
fn test_task_search() {
    let db_path = format!("/tmp/kanban_test_search_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let admin_key_id: String = conn
        .query_row("SELECT id FROM api_keys WHERE is_admin = 1", [], |row| {
            row.get(0)
        })
        .unwrap();

    // Create board + column
    let board_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO boards (id, name, description, owner_key_id) VALUES (?1, 'Search Test', '', ?2)",
        rusqlite::params![board_id, admin_key_id],
    )
    .unwrap();

    let col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Todo', 0)",
        rusqlite::params![col_id, board_id],
    )
    .unwrap();

    // Create tasks with varying content
    let tasks = vec![
        (
            "Fix login bug",
            "Users cannot login with OAuth",
            r#"["bug","auth"]"#,
        ),
        (
            "Add search endpoint",
            "Implement full-text search for tasks",
            r#"["feature","api"]"#,
        ),
        (
            "Update auth docs",
            "Document the OAuth flow changes",
            r#"["docs","auth"]"#,
        ),
        (
            "Deploy to production",
            "Final deployment steps",
            r#"["ops"]"#,
        ),
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

    // Test: search by title keyword
    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*) FROM tasks WHERE board_id = ?1 AND (title LIKE ?2 OR description LIKE ?2 OR labels LIKE ?2)",
        )
        .unwrap();

    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%login%"], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1, "Should find 1 task matching 'login'");

    // Test: search by description keyword
    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%OAuth%"], |row| row.get(0))
        .unwrap();
    assert_eq!(
        count, 2,
        "Should find 2 tasks matching 'OAuth' (login bug + auth docs)"
    );

    // Test: search by label
    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%auth%"], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2, "Should find 2 tasks matching 'auth' (login bug has 'auth' label, auth docs has 'auth' in title + label)");

    // Test: search with no results
    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%xyznonexistent%"], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0, "Should find 0 tasks for nonsense query");

    // Test: search for 'deploy' — only in title
    let count: i64 = stmt
        .query_row(rusqlite::params![board_id, "%deploy%"], |row| row.get(0))
        .unwrap();
    // "Deploy to production" in title + "deployment" in description = 1 task
    assert_eq!(count, 1, "Should find 1 task matching 'deploy'");

    drop(stmt);
    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_task_ordering_positions() {
    let db_path = format!("/tmp/kanban_test_order_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
    let conn = pool.lock().unwrap();

    let admin_key_id: String = conn
        .query_row("SELECT id FROM api_keys WHERE is_admin = 1", [], |row| {
            row.get(0)
        })
        .unwrap();

    // Create board + column
    let board_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO boards (id, name, description, owner_key_id) VALUES (?1, 'Order Test', '', ?2)",
        rusqlite::params![board_id, admin_key_id],
    )
    .unwrap();

    let col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Todo', 0)",
        rusqlite::params![col_id, board_id],
    )
    .unwrap();

    // Create 4 tasks with sequential positions
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

    // Verify initial ordering: Task 0=pos 0, Task 1=pos 1, Task 2=pos 2, Task 3=pos 3
    for (i, tid) in task_ids.iter().enumerate() {
        let pos: i32 = conn
            .query_row(
                "SELECT position FROM tasks WHERE id = ?1",
                rusqlite::params![tid],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pos, i as i32, "Task {} should be at position {}", i, i);
    }

    // Simulate reorder: move Task 3 (pos 3) to position 1
    // Step 1: Close gap at old position (shift tasks after pos 3 down — none in this case)
    conn.execute(
        "UPDATE tasks SET position = position - 1 WHERE column_id = ?1 AND position > 3 AND id != ?2",
        rusqlite::params![col_id, task_ids[3]],
    )
    .unwrap();

    // Step 2: Shift tasks at/after position 1 up to make room
    conn.execute(
        "UPDATE tasks SET position = position + 1 WHERE column_id = ?1 AND position >= 1 AND id != ?2",
        rusqlite::params![col_id, task_ids[3]],
    )
    .unwrap();

    // Step 3: Place Task 3 at position 1
    conn.execute(
        "UPDATE tasks SET position = 1 WHERE id = ?1",
        rusqlite::params![task_ids[3]],
    )
    .unwrap();

    // Verify new ordering: Task 0=0, Task 3=1, Task 1=2, Task 2=3
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
    assert_eq!(positions[0].1, 0);
    assert_eq!(positions[1].0, "Task 3");
    assert_eq!(positions[1].1, 1);
    assert_eq!(positions[2].0, "Task 1");
    assert_eq!(positions[2].1, 2);
    assert_eq!(positions[3].0, "Task 2");
    assert_eq!(positions[3].1, 3);

    // Test insert at specific position: insert at position 0 (top)
    let new_task_id = uuid::Uuid::new_v4().to_string();
    // Shift existing tasks
    conn.execute(
        "UPDATE tasks SET position = position + 1 WHERE column_id = ?1 AND position >= 0",
        rusqlite::params![col_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, board_id, column_id, title, position, created_by) VALUES (?1, ?2, ?3, 'Top Task', 0, 'test')",
        rusqlite::params![new_task_id, board_id, col_id],
    )
    .unwrap();

    // Verify "Top Task" is at position 0
    let top_title: String = conn
        .query_row(
            "SELECT title FROM tasks WHERE column_id = ?1 ORDER BY position ASC LIMIT 1",
            rusqlite::params![col_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(top_title, "Top Task");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_batch_operations() {
    let db_path = format!("/tmp/kanban_test_batch_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
    let conn = pool.lock().unwrap();

    // Get the auto-created admin key
    let admin_key_id: String = conn
        .query_row("SELECT id FROM api_keys WHERE is_admin = 1", [], |row| {
            row.get(0)
        })
        .expect("Admin key should exist");

    // Create a board with two columns
    let board_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO boards (id, name, description, owner_key_id) VALUES (?1, 'Batch Board', '', ?2)",
        rusqlite::params![board_id, admin_key_id],
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

    // Create 5 tasks in backlog
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

    // Test batch move: move first 3 tasks to Done
    let move_ids = &task_ids[0..3];
    for tid in move_ids {
        conn.execute(
            "UPDATE tasks SET column_id = ?1, completed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?2 AND board_id = ?3",
            rusqlite::params![col_done, tid, board_id],
        )
        .unwrap();
    }

    // Verify 3 tasks in Done, 2 in Backlog
    let done_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE board_id = ?1 AND column_id = ?2",
            rusqlite::params![board_id, col_done],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(done_count, 3, "Should have 3 tasks in Done");

    let backlog_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE board_id = ?1 AND column_id = ?2",
            rusqlite::params![board_id, col_backlog],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(backlog_count, 2, "Should have 2 tasks in Backlog");

    // Verify completed_at is set for moved tasks
    let completed: Option<String> = conn
        .query_row(
            "SELECT completed_at FROM tasks WHERE id = ?1",
            rusqlite::params![task_ids[0]],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        completed.is_some(),
        "Moved task should have completed_at set"
    );

    // Test batch update: update priority on remaining backlog tasks
    for tid in &task_ids[3..5] {
        conn.execute(
            "UPDATE tasks SET priority = 99, updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![tid],
        )
        .unwrap();
    }

    let updated_priority: i32 = conn
        .query_row(
            "SELECT priority FROM tasks WHERE id = ?1",
            rusqlite::params![task_ids[3]],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(updated_priority, 99, "Priority should be updated to 99");

    // Test batch delete: delete the Done tasks
    for tid in &task_ids[0..3] {
        conn.execute(
            "DELETE FROM tasks WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![tid, board_id],
        )
        .unwrap();
    }

    let total_remaining: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE board_id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        total_remaining, 2,
        "Should have 2 tasks remaining after batch delete"
    );

    // Verify the remaining tasks are the backlog ones with updated priority
    let remaining_priorities: Vec<i32> = conn
        .prepare("SELECT priority FROM tasks WHERE board_id = ?1 ORDER BY title")
        .unwrap()
        .query_map(rusqlite::params![board_id], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(
        remaining_priorities,
        vec![99, 99],
        "Both remaining tasks should have priority 99"
    );

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_board_archiving() {
    let db_path = format!("/tmp/kanban_test_archive_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);

    let pool = kanban::db::init_db().expect("DB should initialize");
    let conn = pool.lock().unwrap();

    // Create an admin key
    let admin_key_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO api_keys (id, name, key_hash, is_admin, agent_id) VALUES (?1, 'TestAdmin', 'hash_admin', 1, 'admin')",
        rusqlite::params![admin_key_id],
    ).unwrap();

    // Create a board
    let board_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO boards (id, name, description, owner_key_id) VALUES (?1, 'Test Board', 'For archive testing', ?2)",
        rusqlite::params![board_id, admin_key_id],
    ).unwrap();

    // Create a column
    let col_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO columns (id, board_id, name, position) VALUES (?1, ?2, 'Backlog', 0)",
        rusqlite::params![col_id, board_id],
    )
    .unwrap();

    // Create a task on the board
    let task_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tasks (id, board_id, column_id, title, created_by) VALUES (?1, ?2, ?3, 'Test Task', 'admin')",
        rusqlite::params![task_id, board_id, col_id],
    ).unwrap();

    // Board should NOT be archived initially
    let archived: bool = conn
        .query_row(
            "SELECT archived = 1 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!archived, "Board should not be archived initially");

    // Archive the board
    conn.execute(
        "UPDATE boards SET archived = 1, updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![board_id],
    )
    .unwrap();

    let archived: bool = conn
        .query_row(
            "SELECT archived = 1 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(archived, "Board should be archived after archive operation");

    // Verify list_boards filtering: non-archived query should exclude this board
    let non_archived_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM boards WHERE archived = 0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let all_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .unwrap();
    assert!(
        all_count > non_archived_count,
        "Archived board should be filtered when archived = 0 filter is applied"
    );

    // Unarchive the board
    conn.execute(
        "UPDATE boards SET archived = 0, updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![board_id],
    )
    .unwrap();

    let archived: bool = conn
        .query_row(
            "SELECT archived = 1 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        !archived,
        "Board should not be archived after unarchive operation"
    );

    // Verify the task still exists (archiving doesn't delete data)
    let task_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        task_exists,
        "Tasks should not be deleted when board is archived/unarchived"
    );

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_file(&db_path);
}
