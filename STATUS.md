# Kanban - Status

## Current State: Backend API Skeleton ✅

Initial Rust/Rocket + SQLite backend exists, compiles cleanly (clippy -D warnings), and has basic integration tests.

### What's Done

- Rust/Rocket API server (`/api/v1`)
- SQLite schema (WAL mode) with tables:
  - `api_keys`, `boards`, `columns`, `tasks`, `task_events`
- Admin API key auto-generated on first run (printed to stdout)
- Core endpoints:
  - Health: `GET /health`
  - Boards: `POST /boards`, `GET /boards`, `GET /boards/{id}`
  - Columns: `POST /boards/{id}/columns`
  - Tasks: create/list/get/update/delete
  - Agent-first coordination: claim/release/move
  - Task events: list events + add comment
  - API keys (admin): list/create/revoke
- Tests:
  - DB init creates schema + admin key
  - WAL mode enabled
  - Deterministic key hashing

### Key Product Decisions (early)

- **Agent-first claim vs assignment**
  - `assigned_to` = responsibility
  - `claimed_by` = actively working right now (conflict prevention / coordination)
- **SQLite first** for self-hosted simplicity
- **Event log** (`task_events`) is first-class: agents can read history and add comments

### What's Next (Priority Order)

1. **OpenAPI spec** (`/openapi.json`) + documented auth/security schemes
2. **Access control** (board ownership + collaborator model)
3. **WebSocket / SSE event stream** for real-time updates
4. **WIP limits** enforcement per column
5. **Task ordering** improvements (drag/drop positions + stable sorting)
6. **Search** (full-text for title/description/labels)
7. **Human dashboard** (optional)

---

*Last updated: 2026-02-07 08:10 UTC — Session: initial backend + claim/release/move + tests*
