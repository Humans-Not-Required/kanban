# Kanban - Status

## Current State: Backend API Skeleton ✅ + OpenAPI Spec v0.3.0 ✅ + Access Control ✅ + WIP Limits ✅ + Rate Limiting ✅ + Docker ✅ + README Complete ✅

Rust/Rocket + SQLite backend with full OpenAPI 3.0 documentation, board-level access control, WIP limit enforcement, per-key rate limiting with response headers, and Docker deployment. Compiles cleanly (clippy -D warnings), all tests pass (run with `--test-threads=1`).

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
- **Auth:** API key authentication via `Authorization: Bearer` or `X-API-Key` header
- **Database:** SQLite with WAL mode, auto-creates admin key on first run
- **Docker:** Dockerfile (multi-stage build) + docker-compose.yml
- **Config:** Environment variables via `.env` / `dotenvy` (DATABASE_PATH, ROCKET_ADDRESS, ROCKET_PORT, RATE_LIMIT_WINDOW_SECS)
- **Tests:** 13 tests passing (3 lib unit + 3 rate limiter unit + 7 integration)
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

1. **WebSocket / SSE event stream** for real-time updates
2. **Task ordering** improvements (drag/drop positions + stable sorting)
3. **Search** (full-text for title/description/labels)

**Consider deployable?** Core API is feature-complete: boards, columns, tasks, claim/release/move coordination, access control, WIP limits, rate limiting with headers, event logging, comments, OpenAPI spec, Docker support. Tests pass. This is deployable — remaining items are enhancements.

### ⚠️ Gotchas

- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CORS wide open (all origins) — tighten for production
- Admin key printed to stdout on first run — save it!
- OpenAPI spec is at v0.3.0 — 21 paths, 19 schemas, rate limiting fully documented
- WIP limit enforcement uses 409 Conflict — agents should handle this gracefully
- Rate limiter state is in-memory — resets on server restart
- **Tests must run with `--test-threads=1`** — tests use `std::env::set_var("DATABASE_PATH", ...)` which races under parallel execution

### Architecture Notes

- `access.rs` module with `BoardRole` enum using `PartialOrd`/`Ord` for role comparison
- `require_role()` is the single access enforcement point
- `rate_limit.rs` uses `Mutex<HashMap>` with fixed-window algorithm — O(1) per check
- Rate limit headers via Rocket fairing reading request-local state from auth guard
- Single-threaded SQLite via `Mutex<Connection>`
- CORS wide open (all origins)
- Redirect route for short URLs at root (`/`), API routes at `/api/v1`

---

*Last updated: 2026-02-07 10:23 UTC — Session: README update (rate limiting docs, config table, test flags)*
