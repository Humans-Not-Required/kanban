use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub type DbPool = Mutex<Connection>;
pub type WebhookDb = Arc<Mutex<Connection>>;

pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn init_db() -> Result<DbPool, String> {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "kanban.db".to_string());

    let conn = Connection::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

    // Enable WAL mode for better concurrent read performance
    // Retry a few times to handle transient locks during test initialization
    let mut attempts = 0;
    loop {
        match conn.execute_batch("PRAGMA journal_mode=WAL;") {
            Ok(_) => break,
            Err(e) if attempts < 3 => {
                attempts += 1;
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(format!("Failed to set WAL mode: {}", e)),
        }
    }

    conn.execute_batch(
        "
        -- Boards group related tasks
        -- No user accounts: board creation returns a manage_key token.
        -- Anyone with the board UUID can read; manage_key required to write.
        CREATE TABLE IF NOT EXISTS boards (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            manage_key_hash TEXT NOT NULL,
            is_public INTEGER NOT NULL DEFAULT 0,
            archived INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
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
            created_by TEXT NOT NULL DEFAULT '',
            assigned_to TEXT,
            claimed_by TEXT,
            claimed_at TEXT,
            labels TEXT NOT NULL DEFAULT '[]',
            metadata TEXT NOT NULL DEFAULT '{}',
            due_at TEXT,
            completed_at TEXT,
            archived_at TEXT,
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

        -- Webhooks for external notifications
        CREATE TABLE IF NOT EXISTS webhooks (
            id TEXT PRIMARY KEY,
            board_id TEXT NOT NULL,
            url TEXT NOT NULL,
            secret TEXT NOT NULL,
            events TEXT NOT NULL DEFAULT '[]',
            created_by TEXT NOT NULL DEFAULT '',
            active INTEGER NOT NULL DEFAULT 1,
            failure_count INTEGER NOT NULL DEFAULT 0,
            last_triggered_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
        );

        -- Task dependencies (blocks/blocked-by relationships)
        CREATE TABLE IF NOT EXISTS task_dependencies (
            id TEXT PRIMARY KEY,
            board_id TEXT NOT NULL,
            blocker_task_id TEXT NOT NULL,
            blocked_task_id TEXT NOT NULL,
            created_by TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE,
            FOREIGN KEY (blocker_task_id) REFERENCES tasks(id) ON DELETE CASCADE,
            FOREIGN KEY (blocked_task_id) REFERENCES tasks(id) ON DELETE CASCADE,
            UNIQUE(blocker_task_id, blocked_task_id)
        );

        -- Indexes
        CREATE INDEX IF NOT EXISTS idx_tasks_board ON tasks(board_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_column ON tasks(column_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_assigned ON tasks(assigned_to);
        CREATE INDEX IF NOT EXISTS idx_tasks_claimed ON tasks(claimed_by);
        CREATE INDEX IF NOT EXISTS idx_events_task ON task_events(task_id);
        CREATE INDEX IF NOT EXISTS idx_columns_board ON columns(board_id);
        CREATE INDEX IF NOT EXISTS idx_webhooks_board ON webhooks(board_id);
        CREATE INDEX IF NOT EXISTS idx_deps_blocker ON task_dependencies(blocker_task_id);
        CREATE INDEX IF NOT EXISTS idx_deps_blocked ON task_dependencies(blocked_task_id);
        CREATE INDEX IF NOT EXISTS idx_deps_board ON task_dependencies(board_id);
        CREATE INDEX IF NOT EXISTS idx_boards_public ON boards(is_public);
        ",
    )
    .map_err(|e| format!("Failed to create tables: {}", e))?;

    // Migration: add archived_at column to existing databases
    let _ = conn.execute_batch(
        "ALTER TABLE tasks ADD COLUMN archived_at TEXT;"
    );
    // (silently ignored if column already exists)

    Ok(Mutex::new(conn))
}

/// Open a separate database connection for async webhook delivery.
/// Uses WAL mode for concurrent reads alongside the main connection.
pub fn init_webhook_db() -> Result<WebhookDb, String> {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "kanban.db".to_string());

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("Failed to open webhook database: {}", e))?;

    // Retry a few times to handle transient locks during test initialization
    let mut attempts = 0;
    loop {
        match conn.execute_batch("PRAGMA journal_mode=WAL;") {
            Ok(_) => break,
            Err(e) if attempts < 3 => {
                attempts += 1;
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(format!("Failed to set WAL mode for webhook db: {}", e)),
        }
    }

    Ok(Arc::new(Mutex::new(conn)))
}
