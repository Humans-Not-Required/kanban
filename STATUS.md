# Kanban - Status

## Current State: Backend API Skeleton ✅ + OpenAPI Spec ✅ + Access Control ✅

Rust/Rocket + SQLite backend with full OpenAPI 3.0 documentation and board-level access control. Compiles cleanly (clippy -D warnings), all tests pass.

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
- **OpenAPI 3.0 Spec** (v0.1.0):
  - 18 paths with full request/response documentation
  - 14 schemas
  - Tags: System, Boards, Columns, Tasks, Coordination, Events, Admin
- Tests:
  - DB init creates schema + admin key
  - WAL mode enabled
  - Deterministic key hashing
  - board_collaborators table exists with correct schema
  - Access control role logic (owner/admin/collaborator/outsider)

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
- **OpenAPI at v0.1.0** — matches crate version

### What's Next (Priority Order)

1. **WIP limits enforcement** — column WIP limits exist in schema but aren't enforced on task creation/move
2. **Update OpenAPI spec** — add collaborator endpoints and access control documentation
3. **README** — setup instructions, API overview, Docker support
4. **Docker** — Dockerfile + docker-compose.yml for easy deployment
5. **WebSocket / SSE event stream** for real-time updates
6. **Task ordering** improvements (drag/drop positions + stable sorting)
7. **Search** (full-text for title/description/labels)
8. **Rate limiting** — currently no rate limiting (needs rate_limit module like qr-service)

### ⚠️ Gotchas

- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CORS wide open (all origins) — tighten for production
- No rate limiting yet — all requests allowed regardless of rate_limit field in api_keys table
- Admin key printed to stdout on first run — save it!
- OpenAPI spec doesn't yet document collaborator endpoints or access control error responses
- Access checks use `require_role` which checks board existence + role in one call (replaces old `verify_board_exists`)

### Architecture Notes

- `access.rs` module with `BoardRole` enum (Viewer/Editor/Admin/Owner) using `PartialOrd`/`Ord` for role comparison
- `require_role()` is the single enforcement point — checks board exists, then role. Returns structured errors (NO_ACCESS, INSUFFICIENT_ROLE, NOT_FOUND)
- `get_board_role()` checks: admin key → owner → collaborator table (in that order)
- Collaborators table uses `ON CONFLICT DO UPDATE` for upsert semantics
- All routes now use `key: AuthenticatedKey` (not `_key`) since access checks need the key

---

*Last updated: 2026-02-07 09:30 UTC — Session: Board-level access control*
