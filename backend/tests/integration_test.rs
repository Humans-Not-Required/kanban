// Unit tests for kanban service

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
