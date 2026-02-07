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
    assert!(!(BoardRole::Viewer >= BoardRole::Editor));

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
