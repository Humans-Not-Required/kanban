# Kanban - Status

## Current State: Auth Refactor Complete ✅

Per-board token auth model implemented. Zero-signup, link-based access control.

### Auth Model (NEW)

| Operation | Auth Required | How |
|-----------|--------------|-----|
| Create board | ❌ No | Returns `manage_key` (shown once) |
| View board/tasks/events/deps | ❌ No | Just need board UUID |
| List public boards | ❌ No | Shows `is_public=true` boards |
| Write (create/update/delete tasks, columns, webhooks) | 🔑 manage_key | Bearer header, X-API-Key, or `?key=` query param |
| Archive/unarchive board | 🔑 manage_key | Same as above |

### What's Done

- **Auth refactor** — per-board tokens replacing global API keys
  - `POST /boards` returns `manage_key`, `view_url`, `manage_url`, `api_base`
  - `BoardToken` request guard extracts token from 3 sources (Bearer, X-API-Key, ?key=)
  - Read routes are fully public (just need board UUID)
  - Write routes verify manage_key hash against board
  - Removed: global API keys (/keys CRUD), collaborator system, per-key rate limiting
  - Added: `is_public` flag, `actor_name` fields, `?agent=` on claim
- **Frontend auth integration** — per-board key detection and edit/view modes
  - Detects `?key=` in URL, stores in localStorage per board, cleans URL
  - Edit/View mode badge in header and board view
  - Board creation shows manage URL + view URL + API base with copy buttons
  - Read-only mode hides edit controls (new task button, drag-drop)
  - No global API key required — app loads directly
  - Sidebar shows public boards + direct board ID/URL input
  - `is_public` toggle in board creation modal
- **Core API** — all routes working with new auth model
- **Frontend** — React + Vite dashboard with drag-and-drop
- **Docker** — 3-stage multi-stage build
- **Tests** — 17 passing (3 unit + 14 integration), zero clippy warnings
- **Deployed** — kanban.ckbdev.com via Cloudflare Tunnel

### Tech Stack

- Rust 1.83+ / Rocket 0.5 / SQLite (rusqlite)
- React + Vite frontend, unified serving on single port
- CORS: wide open (all origins) — tighten for production

### What's Next (Priority Order)

1. ~~**Deploy updated backend + frontend**~~ ✅ Done (2026-02-07 22:32 UTC)
2. ~~**Comments visible in frontend**~~ ✅ Done (2026-02-07 22:35 UTC) — task detail modal with comments, activity log, add comment form
3. ~~**Identity on actions**~~ ✅ Done (2026-02-07 23:04 UTC) — persistent display name in header, sent with all write ops (create/update/comment/claim)
4. ~~**Task editing in frontend**~~ ✅ Done (2026-02-07 23:04 UTC) — edit button in task detail modal, inline form for title/desc/priority/labels/assignment, delete with confirmation
5. ~~**IP-based rate limiting for board creation**~~ ✅ Done (2026-02-07 23:35 UTC) — ClientIp guard (XFF/X-Real-Ip/socket), 10 boards/hr/IP default, configurable via BOARD_RATE_LIMIT env var, 429 with RATE_LIMIT_EXCEEDED code
6. ~~**Desktop move-to-column in detail modal**~~ ✅ Done (2026-02-07 23:36 UTC) — removed isMobile guard, now available on all screen sizes
7. **Real-time updates via SSE** — connect to `/boards/{id}/events/stream` for live task changes
8. **Add HTTP integration tests** — current tests are unit/DB-level; add Rocket test client tests for rate limiting, auth guards, etc.

### ⚠️ Gotchas

- **Breaking DB change** — new schema has no `api_keys` table. Fresh DB required. Old DBs will not work.
- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CORS wide open (all origins) — tighten for production
- **Tests must run with `--test-threads=1`** — tests use `std::env::set_var("DATABASE_PATH", ...)` which races under parallel execution
- Rate limiter now active on board creation (10/hr/IP default, configurable via BOARD_RATE_LIMIT env var)

### Architecture Notes

- `auth.rs` — `BoardToken` request guard extracts token from Bearer/X-API-Key/?key=
- `access.rs` — `require_manage_key()`, `require_board_exists()`, `require_not_archived()`
- `routes.rs` — all write routes take `BoardToken`, hash it, verify against board's `manage_key_hash`
- `db.rs` — `boards` table has `manage_key_hash` and `is_public` columns
- No user/account system — boards are the only resource, tokens are per-board
- Single-threaded SQLite via `Mutex<Connection>`

### Key Product Decisions

- **Pastebin/Excalidraw model** — create board → get management URL, share with others
- **View URL** = read-only, **Manage URL** = full access
- **Unlisted by default** — boards are accessible by UUID but not discoverable unless `is_public=true`
- **actor_name is optional free text** — no identity verification, trust-based
- **Claim vs assignment** preserved — `claimed_by` = actively working, `assigned_to` = responsibility

---

*Last updated: 2026-02-07 23:36 UTC — Session: IP-based rate limiting on board creation (ClientIp guard, 10/hr/IP, 429 response). Move-to-column dropdown now available on desktop (was mobile-only). 22 tests passing (4 lib + 4 bin + 14 integration), zero clippy warnings. Both changes deployed to staging.*
