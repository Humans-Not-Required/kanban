use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::sync::Mutex;

pub type DbPool = Mutex<Connection>;

pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn init_db() -> Result<DbPool, String> {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "kanban.db".to_string());

    let conn = Connection::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| format!("Failed to set WAL mode: {}", e))?;

    conn.execute_batch(
        "
        -- API keys for authentication
        CREATE TABLE IF NOT EXISTS api_keys (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL UNIQUE,
            is_admin INTEGER NOT NULL DEFAULT 0,
            agent_id TEXT,
            rate_limit INTEGER NOT NULL DEFAULT 100,
            active INTEGER NOT NULL DEFAULT 1,
            requests_count INTEGER NOT NULL DEFAULT 0,
            last_used_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- Boards group related tasks
        CREATE TABLE IF NOT EXISTS boards (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            owner_key_id TEXT NOT NULL,
            archived INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (owner_key_id) REFERENCES api_keys(id)
        );

        -- Columns define workflow stages within a board
        CREATE TABLE IF NOT EXISTS columns (
            id TEXT PRIMARY KEY,
            board_id TEXT NOT NULL,
            name TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            wip_limit INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
        );

        -- Tasks are the core work items
        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            board_id TEXT NOT NULL,
            column_id TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 0,
            position INTEGER NOT NULL DEFAULT 0,
            created_by TEXT NOT NULL,
            assigned_to TEXT,
            claimed_by TEXT,
            claimed_at TEXT,
            labels TEXT NOT NULL DEFAULT '[]',
            metadata TEXT NOT NULL DEFAULT '{}',
            due_at TEXT,
            completed_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE,
            FOREIGN KEY (column_id) REFERENCES columns(id)
        );

        -- Task comments / activity log
        CREATE TABLE IF NOT EXISTS task_events (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            actor TEXT NOT NULL,
            data TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );

        -- Board collaborators for access control
        CREATE TABLE IF NOT EXISTS board_collaborators (
            board_id TEXT NOT NULL,
            key_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'editor',
            added_by TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (board_id, key_id),
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE,
            FOREIGN KEY (key_id) REFERENCES api_keys(id)
        );

        -- Indexes
        CREATE INDEX IF NOT EXISTS idx_tasks_board ON tasks(board_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_column ON tasks(column_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_assigned ON tasks(assigned_to);
        CREATE INDEX IF NOT EXISTS idx_tasks_claimed ON tasks(claimed_by);
        CREATE INDEX IF NOT EXISTS idx_events_task ON task_events(task_id);
        CREATE INDEX IF NOT EXISTS idx_columns_board ON columns(board_id);
        CREATE INDEX IF NOT EXISTS idx_collaborators_key ON board_collaborators(key_id);
        ",
    )
    .map_err(|e| format!("Failed to create tables: {}", e))?;

    // Create admin key if none exists
    let key_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM api_keys", [], |row| row.get(0))
        .unwrap_or(0);

    if key_count == 0 {
        let admin_key = format!(
            "kb_admin_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "")
        );
        let key_hash = hash_key(&admin_key);
        let id = uuid::Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO api_keys (id, name, key_hash, is_admin, agent_id) VALUES (?1, ?2, ?3, 1, 'admin')",
            rusqlite::params![id, "Admin", key_hash],
        )
        .map_err(|e| format!("Failed to create admin key: {}", e))?;

        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  ADMIN API KEY (save this — it won't be shown again):   ║");
        println!("║  {}  ║", admin_key);
        println!("╚══════════════════════════════════════════════════════════╝");
    }

    Ok(Mutex::new(conn))
}
