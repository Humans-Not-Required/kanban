# Kanban - Status

## Current State: Backend API Skeleton ✅ + OpenAPI Spec v0.7.0 ✅ + Access Control ✅ + WIP Limits ✅ + Rate Limiting ✅ + SSE Events ✅ + Task Reorder ✅ + Task Search ✅ + Batch Operations ✅ + Docker ✅ + README Complete ✅

Rust/Rocket + SQLite backend with full OpenAPI 3.0 documentation, board-level access control, WIP limit enforcement, per-key rate limiting with response headers, task reorder/positioning, full-text search, batch operations, and Docker deployment. Compiles cleanly (clippy -D warnings), all tests pass (run with `--test-threads=1`).

### What's Done

- **Core API** (all routes implemented):
  - `POST /boards` — Create board with custom columns
  - `GET /boards` — List boards (scoped by access)
  - `GET /boards/{id}` — Board details with columns and task counts
  - `POST /boards/{id}/columns` — Add column (Admin+)
  - `POST /boards/{id}/tasks` — Create task (Editor+)
  - `GET /boards/{id}/tasks` — List tasks with filters (Viewer+)
  - `GET /boards/{id}/tasks/{id}` — Get task (Viewer+)
  - `PATCH /boards/{id}/tasks/{id}` — Update task (Editor+)
  - `DELETE /boards/{id}/tasks/{id}` — Delete task (Editor+)
  - `POST .../tasks/{id}/claim` — Claim task (Editor+)
  - `POST .../tasks/{id}/release` — Release claim (Editor+)
  - `POST .../tasks/{id}/move/{col}` — Move task (Editor+)
  - `GET .../tasks/{id}/events` — Task event log (Viewer+)
  - `POST .../tasks/{id}/comment` — Comment (Viewer+)
  - `GET /boards/{id}/collaborators` — List collaborators (Viewer+)
  - `POST /boards/{id}/collaborators` — Add/update collaborator (Admin+)
  - `DELETE /boards/{id}/collaborators/{keyId}` — Remove collaborator (Admin+)
  - `GET /keys` — List API keys (admin only)
  - `POST /keys` — Create API key (admin only)
  - `DELETE /keys/{id}` — Revoke API key (admin only)
  - `GET /health` — Health check
  - `GET /openapi.json` — OpenAPI 3.0 spec (v0.3.0)
- **Access Control:**
  - Role hierarchy: Viewer < Editor < Admin < Owner
  - Board owner = implicit full access (via `owner_key_id`)
  - Global admin API keys = full access to all boards
  - Collaborator management with upsert semantics
- **WIP Limit Enforcement:**
  - `check_wip_limit()` validates column capacity before adding/moving tasks
  - Returns 409 Conflict with `WIP_LIMIT_EXCEEDED` error code
  - Columns with `wip_limit = NULL` are unlimited
- **Rate Limiting (NEW):**
  - Fixed-window per-key enforcement via in-memory rate limiter
  - Each API key has a configurable `rate_limit` (requests per window)
  - Default: 100 req/min for regular keys
  - Window duration configurable via `RATE_LIMIT_WINDOW_SECS` env var (default: 60s)
  - Returns 429 Too Many Requests when limit exceeded
  - Response headers on ALL authenticated requests:
    - `X-RateLimit-Limit` — max requests in current window
    - `X-RateLimit-Remaining` — requests remaining
    - `X-RateLimit-Reset` — seconds until window resets
  - Implemented via auth guard (single enforcement point) + Rocket fairing (headers)
  - Zero database overhead — all tracking is in-memory
- **Batch Operations (NEW):**
  - `POST /boards/{id}/tasks/batch` — Execute multiple operations in one request
    - `move` — Move tasks to a different column (handles done-column completion)
    - `update` — Update fields (priority, assigned_to, labels, due_at) on multiple tasks
    - `delete` — Delete multiple tasks
    - Max 50 operations per request
    - Independent execution — failures in one don't affect others
    - Per-operation result with success/failure, error messages, and affected count
    - SSE events emitted for each individual task change (tagged with `batch: true`)
    - Integration test covering move, update, and delete flows
- **Task Search:**
  - `GET /boards/{id}/tasks/search?q=<query>` — full-text search across title, description, labels
  - Relevance ranking: title matches first, then by priority DESC, then by updated_at DESC
  - Pagination via `limit` (1-100, default 50) and `offset`, with total count in response
  - Combinable filters: `column`, `assigned`, `priority`, `label`
  - Returns `SearchResponse` with query, tasks, total, limit, offset
  - Integration test with title/description/label matching coverage
- **Task Reorder/Positioning:**
  - `POST /boards/{id}/tasks/{taskId}/reorder` — set task position within column
  - Optional `column_id` for move+reorder in one call
  - Shift-based positioning: tasks at/after target position move down automatically
  - Same-column reorder closes gap at old position first
  - Cross-column reorder checks WIP limits and sets completed_at for done columns
  - `CreateTaskRequest` accepts optional `position` field for insert-at
  - SSE event type: `task.reordered`
  - Integration test: task ordering with reorder and insert-at-position
- **SSE Real-Time Events:**
  - `GET /boards/{id}/events/stream` — Server-Sent Events stream (Viewer+)
  - EventBus using `tokio::sync::broadcast` channels per board (lazy creation)
  - 7 event types: task.created, task.updated, task.deleted, task.claimed, task.released, task.moved, task.comment
  - 15-second heartbeat to keep connections alive
  - Graceful lagged-client handling (warning event if >256 events buffered)
  - Channel capacity: 256 events per board
  - No persistence — events are fire-and-forget to connected subscribers
- **Auth:** API key authentication via `Authorization: Bearer` or `X-API-Key` header
- **Database:** SQLite with WAL mode, auto-creates admin key on first run
- **Docker:** Dockerfile (multi-stage build) + docker-compose.yml
- **Config:** Environment variables via `.env` / `dotenvy` (DATABASE_PATH, ROCKET_ADDRESS, ROCKET_PORT, RATE_LIMIT_WINDOW_SECS)
- **Tests:** 16 tests passing (3 access control unit + 3 rate limiter unit + 10 integration)
- **Code Quality:** Zero clippy warnings, cargo fmt clean

### Tech Stack

- Rust 1.83+ / Rocket 0.5 / SQLite (rusqlite)
- CORS: wide open (all origins) — tighten for production

### Key Product Decisions

- **Agent-first claim vs assignment**
  - `assigned_to` = responsibility
  - `claimed_by` = actively working right now (conflict prevention / coordination)
- **SQLite first** for self-hosted simplicity
- **Event log** (`task_events`) is first-class: agents can read history and add comments
- **Role-based access per board** — Owner/Admin/Editor/Viewer hierarchy
- **In-memory rate limiter** — no DB overhead per request; resets on restart (acceptable trade-off)
- **Rate limit check in auth guard** — single enforcement point; all authenticated routes covered automatically

### What's Next (Priority Order)

1. ~~**Batch operations**~~ ✅ Done
2. **Board archiving** (archive/unarchive boards via API)
3. **Webhooks** (notify external URLs on task events)

**Consider deployable?** Core API is feature-complete: boards, columns, tasks, claim/release/move coordination, access control, WIP limits, rate limiting with headers, SSE real-time events, full-text search, event logging, comments, OpenAPI spec, Docker support. Tests pass. This is deployable — remaining items are enhancements.

### ⚠️ Gotchas

- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CORS wide open (all origins) — tighten for production
- Admin key printed to stdout on first run — save it!
- OpenAPI spec is at v0.7.0 — 19 paths, batch + search endpoints + BatchRequest/Response/Operation/Result schemas documented
- WIP limit enforcement uses 409 Conflict — agents should handle this gracefully
- Rate limiter state is in-memory — resets on server restart
- **Tests must run with `--test-threads=1`** — tests use `std::env::set_var("DATABASE_PATH", ...)` which races under parallel execution

### Architecture Notes

- `access.rs` module with `BoardRole` enum using `PartialOrd`/`Ord` for role comparison
- `require_role()` is the single access enforcement point
- `rate_limit.rs` uses `Mutex<HashMap>` with fixed-window algorithm — O(1) per check
- Rate limit headers via Rocket fairing reading request-local state from auth guard
- `events.rs` — EventBus with `Mutex<HashMap<String, broadcast::Sender>>` (lazy per-board channels)
- SSE stream uses `rocket::response::stream::EventStream` with `tokio::select!` for graceful shutdown
- Single-threaded SQLite via `Mutex<Connection>`
- CORS wide open (all origins)
- Redirect route for short URLs at root (`/`), API routes at `/api/v1`

---

*Last updated: 2026-02-07 11:47 UTC — Session: Batch task operations (move/update/delete)*
