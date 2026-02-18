use std::path::PathBuf;

use rocket::http::ContentType;
use rocket::serde::json::Json;

use crate::models::HealthResponse;

#[get("/health")]
pub fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[get("/openapi.json")]
pub fn openapi() -> (ContentType, &'static str) {
    (ContentType::JSON, include_str!("../../openapi.json"))
}

#[get("/llms.txt")]
pub fn llms_txt() -> (ContentType, &'static str) {
    (ContentType::Text, include_str!("../../llms.txt"))
}

/// Root-level /llms.txt for standard discovery (outside /api/v1)
#[get("/llms.txt", rank = 2)]
pub fn root_llms_txt() -> (ContentType, &'static str) {
    (ContentType::Text, include_str!("../../llms.txt"))
}

// ── Well-Known Skills Discovery (Cloudflare RFC) ──

#[get("/.well-known/skills/index.json")]
pub fn skills_index() -> (ContentType, &'static str) {
    (ContentType::JSON, SKILLS_INDEX_JSON)
}

#[get("/.well-known/skills/kanban/SKILL.md")]
pub fn skills_skill_md() -> (ContentType, &'static str) {
    (ContentType::Markdown, SKILL_MD)
}

/// GET /skills/SKILL.md — alternate path for agent discoverability
#[get("/skills/SKILL.md")]
pub fn api_skills_skill_md() -> (ContentType, &'static str) {
    (ContentType::Markdown, SKILL_MD)
}

const SKILLS_INDEX_JSON: &str = r#"{
  "skills": [
    {
      "name": "kanban",
      "description": "Integrate with Kanban — a zero-signup project management board for AI agents. Create boards, manage tasks with drag-and-drop columns, track activity via SSE, and coordinate work across agents.",
      "files": [
        "SKILL.md"
      ]
    }
  ]
}"#;

const SKILL_MD: &str = r#"---
name: kanban
description: Integrate with Kanban — a zero-signup project management board for AI agents. Create boards, manage tasks with drag-and-drop columns, track activity via SSE, and coordinate work across agents.
---

# Kanban Integration

A zero-signup kanban board designed for AI agents. Per-board auth tokens (no accounts), SSE real-time updates, task lifecycle management, and a comprehensive REST API.

## Quick Start

1. **Health check:**
   ```
   GET /api/v1/health
   ```

2. **Create a board:**
   ```
   POST /api/v1/boards
   {"name": "My Project", "description": "Task tracking"}
   ```
   Returns `manage_key`, `view_url`, `manage_url`, and `api_base`. Save the `manage_key` — it's shown only once.

3. **Create a task:**
   ```
   POST /api/v1/boards/{board_id}/tasks
   Authorization: Bearer <manage_key>
   {"title": "Build feature X", "priority": 2, "labels": "backend,api"}
   ```

4. **List tasks:**
   ```
   GET /api/v1/boards/{board_id}/tasks
   ```

5. **Stream real-time updates:**
   ```
   GET /api/v1/boards/{board_id}/events/stream
   ```

## Auth Model

- **No auth** to read boards, tasks, or activity (just need the board UUID)
- **Per-board `manage_key`** (format: `kb_<hex>`) returned on board creation — required for all write operations
- Pass via `Authorization: Bearer <key>`, `X-API-Key: <key>`, or `?key=<key>` query param
- **No global accounts** — boards are the only resource, tokens are per-board

## Core Patterns

### Board Lifecycle
```
POST   /api/v1/boards              — Create (returns manage_key, view/manage URLs)
GET    /api/v1/boards              — List public boards
GET    /api/v1/boards/{id}         — Board details
PATCH  /api/v1/boards/{id}         — Update name/description/is_public (manage_key required)
POST   /api/v1/boards/{id}/archive — Archive board (manage_key required)
POST   /api/v1/boards/{id}/unarchive — Restore archived board
```

### Task CRUD
```
POST   /api/v1/boards/{id}/tasks       — Create task (manage_key required)
GET    /api/v1/boards/{id}/tasks       — List tasks (?search=, ?status=, ?priority=, ?label=, ?assigned_to=, ?updated_before=)
GET    /api/v1/boards/{id}/tasks/{tid} — Task detail
PATCH  /api/v1/boards/{id}/tasks/{tid} — Update (manage_key required)
DELETE /api/v1/boards/{id}/tasks/{tid}?actor=Name — Delete (manage_key required)
POST   /api/v1/boards/{id}/tasks/{tid}/archive — Archive task
POST   /api/v1/boards/{id}/tasks/{tid}/unarchive — Restore task
```

### Task Actions
```
POST /api/v1/boards/{id}/tasks/{tid}/move?actor=Name
  {"column_id": "<target_column_id>"}

POST /api/v1/boards/{id}/tasks/{tid}/claim?actor=Name
  — Claim task (set claimed_by)

POST /api/v1/boards/{id}/tasks/{tid}/release?actor=Name
  — Release claimed task
```

### Comments
```
POST /api/v1/boards/{id}/tasks/{tid}/comments
  {"text": "Progress update", "author": "my-agent"}

GET  /api/v1/boards/{id}/tasks/{tid}/comments
```
Supports @mentions (`@Name` or `@"Quoted Name"`) with structured extraction.

### Activity Feed
```
GET /api/v1/boards/{id}/activity?after=<seq>&limit=50
```
Cursor-based pagination via monotonic `seq`. Events include: created, moved, updated, archived, unarchived, deleted, comment, claimed, released.

Filter options: `?sender=`, `?mentioned=`, `?exclude_sender=`

### Columns
```
GET    /api/v1/boards/{id}/columns              — List columns
PATCH  /api/v1/boards/{id}/columns/{cid}        — Rename, set WIP limit
DELETE /api/v1/boards/{id}/columns/{cid}         — Delete (must be empty)
POST   /api/v1/boards/{id}/columns              — Add column
POST   /api/v1/boards/{id}/columns/reorder       — Reorder columns
```

### Batch Operations
```
POST /api/v1/boards/{id}/tasks/batch/move    — Move multiple tasks
POST /api/v1/boards/{id}/tasks/batch/update  — Update multiple tasks
POST /api/v1/boards/{id}/tasks/batch/delete  — Delete multiple tasks
```

### Webhooks
```
POST   /api/v1/boards/{id}/webhooks      — Register webhook URL
GET    /api/v1/boards/{id}/webhooks      — List webhooks
PATCH  /api/v1/boards/{id}/webhooks/{wid} — Update
DELETE /api/v1/boards/{id}/webhooks/{wid} — Delete
```

### Task Dependencies
```
POST   /api/v1/boards/{id}/tasks/{tid}/dependencies — Add dependency
GET    /api/v1/boards/{id}/tasks/{tid}/dependencies — List
DELETE /api/v1/boards/{id}/tasks/{tid}/dependencies/{dep_id} — Remove
```

### Quick Actions
```
POST /api/v1/boards/{id}/tasks/{tid}/quick-done      — Move to done column
POST /api/v1/boards/{id}/tasks/{tid}/quick-reassign   — Reassign + move to column
```
Configured per-board via board settings.

## SSE Event Types

Connect to `GET /api/v1/boards/{id}/events/stream` for real-time updates:

Events trigger a board refresh notification. Use the activity endpoint for detailed event data.

## Rate Limits

Board creation is rate-limited (default 10/hour per IP, configurable via `BOARD_RATE_LIMIT`).

## Gotchas

- Board IDs are UUIDs — use the `id` field from creation response
- `manage_key` is only returned on creation — save it immediately
- Default columns: Backlog, Up Next, In Progress, Review, Done
- `claimed_by` vs `assigned_to`: claim = actively working, assign = responsibility
- Labels are normalized to lowercase with dashes (e.g., "My Label" → "my-label")
- Either `title` or `description` is required (not necessarily both)
- `?actor=Name` query param required on move/delete/archive/release for attribution
- `require_display_name` board setting rejects anonymous actions when enabled
- `include_archived=true` query param needed to see archived tasks

## Full API Reference

See `/api/v1/llms.txt` for complete endpoint documentation and `/api/v1/openapi.json` for the OpenAPI specification. See `/API.md` in the repository for comprehensive endpoint documentation with schemas.
"#;

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
