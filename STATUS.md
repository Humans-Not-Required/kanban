# Kanban - Status

## Current State: Backend API Skeleton ✅ + OpenAPI Spec ✅

Rust/Rocket + SQLite backend with full OpenAPI 3.0 documentation. Compiles cleanly (clippy -D warnings), all tests pass.

### What's Done

- Rust/Rocket API server (`/api/v1`)
- SQLite schema (WAL mode) with tables:
  - `api_keys`, `boards`, `columns`, `tasks`, `task_events`
- Admin API key auto-generated on first run (printed to stdout)
- Core endpoints:
  - Health: `GET /health`
  - OpenAPI: `GET /openapi.json`
  - Boards: `POST /boards`, `GET /boards`, `GET /boards/{id}`
  - Columns: `POST /boards/{id}/columns`
  - Tasks: create/list/get/update/delete
  - Agent-first coordination: claim/release/move
  - Task events: list events + add comment
  - API keys (admin): list/create/revoke
- **OpenAPI 3.0 Spec** (NEW):
  - 18 paths with full request/response documentation
  - 14 schemas (BoardResponse, TaskResponse, etc.)
  - Reusable parameters (boardId, taskId) and error responses
  - Tags: System, Boards, Columns, Tasks, Coordination, Events, Admin
  - Served at GET /api/v1/openapi.json via `include_str!`
- Tests:
  - DB init creates schema + admin key
  - WAL mode enabled
  - Deterministic key hashing

### Tech Stack

- Rust 1.83+ / Rocket 0.5 / SQLite (rusqlite)
- CORS: wide open (all origins) — tighten for production

### Key Product Decisions

- **Agent-first claim vs assignment**
  - `assigned_to` = responsibility
  - `claimed_by` = actively working right now (conflict prevention / coordination)
- **SQLite first** for self-hosted simplicity
- **Event log** (`task_events`) is first-class: agents can read history and add comments
- **OpenAPI at v0.1.0** — matches crate version

### What's Next (Priority Order)

1. **Access control** — board ownership + collaborator model (currently any key can access any board)
2. **WIP limits enforcement** — column WIP limits exist in schema but aren't enforced on task creation/move
3. **README** — setup instructions, API overview, Docker support
4. **Docker** — Dockerfile + docker-compose.yml for easy deployment
5. **WebSocket / SSE event stream** for real-time updates
6. **Task ordering** improvements (drag/drop positions + stable sorting)
7. **Search** (full-text for title/description/labels)
8. **Rate limiting** — currently no rate limiting (needs rate_limit module like qr-service)

### ⚠️ Gotchas

- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CORS wide open (all origins) — tighten for production
- No access control yet — any authenticated key can read/modify any board
- No rate limiting yet — all requests allowed regardless of rate_limit field in api_keys table
- Admin key printed to stdout on first run — save it!

---

*Last updated: 2026-02-07 09:22 UTC — Session: OpenAPI spec implementation*
