# Kanban - Status

## Current State: Backend API Skeleton ✅ + OpenAPI Spec v0.2.0 ✅ + Access Control ✅ + WIP Limits ✅ + Docker ✅

Rust/Rocket + SQLite backend with full OpenAPI 3.0 documentation, board-level access control, WIP limit enforcement, and Docker deployment. Compiles cleanly (clippy -D warnings), all tests pass (run with `--test-threads=1`).

### What's Done

- Rust/Rocket API server (`/api/v1`)
- SQLite schema (WAL mode) with tables:
  - `api_keys`, `boards`, `columns`, `tasks`, `task_events`, `board_collaborators`
- Admin API key auto-generated on first run (printed to stdout)
- **Access Control** (NEW):
  - Role hierarchy: Viewer < Editor < Admin < Owner
  - Board owner = implicit full access (via `owner_key_id`)
  - Global admin API keys = full access to all boards
  - Collaborator management:
    - `GET /boards/{id}/collaborators` — list collaborators (requires Viewer)
    - `POST /boards/{id}/collaborators` — add/update collaborator (requires Admin)
    - `DELETE /boards/{id}/collaborators/{key_id}` — remove collaborator (requires Admin)
  - **WIP Limit Enforcement** (NEW):
    - `check_wip_limit()` helper validates column capacity before adding tasks
    - Enforced on: `create_task`, `move_task`, `update_task` (when column_id changes)
    - Returns 409 Conflict with `WIP_LIMIT_EXCEEDED` error code and column name
    - Excludes the task being moved from count (prevents false positives on moves)
    - Columns with `wip_limit = NULL` are unlimited (no enforcement)
  - Role enforcement on all routes:
    - **Viewer:** read boards, list tasks, get task, view events, post comments
    - **Editor:** create/update/delete tasks, claim/release/move tasks
    - **Admin:** create columns, manage collaborators
    - **Owner:** all of the above (implicit, not stored in collaborators table)
  - Upsert semantics: adding an existing collaborator updates their role
  - Can't add board owner as collaborator (already has Owner role)
  - `list_boards` includes boards the user owns, collaborates on, or has tasks in
- Core endpoints:
  - Health: `GET /health`
  - OpenAPI: `GET /openapi.json`
  - Boards: `POST /boards`, `GET /boards`, `GET /boards/{id}`
  - Columns: `POST /boards/{id}/columns`
  - Tasks: create/list/get/update/delete
  - Agent-first coordination: claim/release/move
  - Task events: list events + add comment
  - API keys (admin): list/create/revoke
- **OpenAPI 3.0 Spec** (v0.2.0):
  - 21 paths with full request/response documentation
  - 18 schemas (including AddCollaboratorRequest, CollaboratorResponse)
  - Tags: System, Boards, Columns, Access Control, Tasks, Coordination, Events, Admin
  - Access control documented: role hierarchy, 403 error codes (NO_ACCESS, INSUFFICIENT_ROLE), role requirements on every endpoint
  - WIP limit enforcement documented with 409 Conflict responses
  - All error codes enumerated in ApiError schema
- Tests (6 passing):
  - DB init creates schema + admin key
  - WAL mode enabled
  - Deterministic key hashing
  - board_collaborators table exists with correct schema
  - Access control role logic (owner/admin/collaborator/outsider)
  - WIP limit enforcement (column schema, limit storage, task counts)

- **Docker:** Multi-stage Dockerfile + docker-compose.yml
  - Builder: `rust:1.83-slim-bookworm`, runtime: `debian:bookworm-slim`
  - Non-root user, healthcheck on `/api/v1/health`, named volume for SQLite data
  - `.env.example` with configuration reference

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
  - Owner is implicit (board creator), never stored in collaborators table
  - Global admin keys bypass all board-level checks
  - Viewer can comment (lightweight contribution) but can't modify tasks
- **OpenAPI at v0.2.0** — separate from crate version, tracks API evolution

### What's Done (This Session)

- Docker support: multi-stage Dockerfile, docker-compose.yml, .env.example
- Fixed README docker build context path and volume mount
- Added .env to .gitignore

### What's Next (Priority Order)

1. **Rate limiting** — port fixed-window in-memory rate limiter from qr-service; enforce on all authenticated routes
2. **WebSocket / SSE event stream** for real-time updates
3. **Task ordering** improvements (drag/drop positions + stable sorting)
4. **Search** (full-text for title/description/labels)

### ⚠️ Gotchas

- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CORS wide open (all origins) — tighten for production
- No rate limiting yet — all requests allowed regardless of rate_limit field in api_keys table
- Admin key printed to stdout on first run — save it!
- OpenAPI spec is at v0.2.0 — 21 paths, 18 schemas, access control fully documented
- WIP limit enforcement uses 409 Conflict — agents should handle this gracefully (move tasks out of full columns first)
- Access checks use `require_role` which checks board existence + role in one call (replaces old `verify_board_exists`)
- **Tests must run with `--test-threads=1`** — tests use `std::env::set_var("DATABASE_PATH", ...)` which races under parallel execution. Use `cargo test -- --test-threads=1`

### Architecture Notes

- `access.rs` module with `BoardRole` enum (Viewer/Editor/Admin/Owner) using `PartialOrd`/`Ord` for role comparison
- `require_role()` is the single enforcement point — checks board exists, then role. Returns structured errors (NO_ACCESS, INSUFFICIENT_ROLE, NOT_FOUND)
- `get_board_role()` checks: admin key → owner → collaborator table (in that order)
- Collaborators table uses `ON CONFLICT DO UPDATE` for upsert semantics
- All routes now use `key: AuthenticatedKey` (not `_key`) since access checks need the key

---

*Last updated: 2026-02-07 10:05 UTC — Session: Docker support (Dockerfile + docker-compose.yml)*
