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
- **Column management** — full CRUD for columns from the frontend
  - `PATCH /boards/{id}/columns/{col_id}` — rename, update WIP limit
  - `DELETE /boards/{id}/columns/{col_id}` — delete empty columns (prevents last column deletion)
  - `POST /boards/{id}/columns/reorder` — reorder via ordered ID list
  - Frontend: double-click to inline rename, ⚙️ menu (rename, move left/right, delete), "+" add column button
- **Core API** — all routes working with new auth model
- **Frontend** — React + Vite dashboard with drag-and-drop
- **Docker** — 3-stage multi-stage build
- **Tests** — 44 passing (14 DB/unit integration + 30 HTTP integration), zero clippy warnings
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
7. ~~**Real-time updates via SSE**~~ ✅ Done (2026-02-08 00:06 UTC) — frontend subscribes to `/boards/{id}/events/stream`, debounced 300ms refresh, auto-reconnect with exponential backoff, live connection indicator (green pulsing dot)
8. ~~**Add HTTP integration tests**~~ ✅ Done (2026-02-08 00:06 UTC) — 20 Rocket test client tests covering board CRUD, auth guards (Bearer/X-API-Key/?key=), task CRUD, move/claim/release, comments, archive/unarchive, search, rate limiting
9. ~~**Column management in frontend**~~ ✅ Done (2026-02-08 00:36 UTC) — Backend: PATCH/DELETE/reorder endpoints + 7 tests. Frontend: inline rename, ⚙️ menu, add column button.
10. ~~**Modal positioning updates**~~ ✅ Done (2026-02-08 01:40 UTC) — upper third (8vh top padding) on desktop/tablet, full viewport on mobile
11. ~~**Webhook management in frontend**~~ ✅ Done (2026-02-08 01:45 UTC) — WebhookManagerModal was already built, just needed wiring (render was missing from JSX)
12. ~~**Board settings panel**~~ ✅ Done (2026-02-08 01:50 UTC) — PATCH /boards/{id} endpoint + BoardSettingsModal (name, desc, is_public) + 3 HTTP tests (30 total)
13. ~~**Improved task filtering**~~ ✅ Done (2026-02-08 02:04 UTC) — filter bar with priority, label, and assignee dropdowns; highlights when active; clear button
14. **Auto-fill fields for human-created tasks** — monitoring agent should set priority/labels/assignment based on title+description (Jordan request, 2026-02-08). (This is handled by the Kanban Board Monitor cron job, not the app UI.)
15. ~~**Collapsible sidebar on tablet**~~ ✅ Done (2026-02-08 02:04 UTC) — sidebar collapses on screens < 1024px (was mobile-only at 768px)
16. ~~**Bigger description fields**~~ ✅ Done (2026-02-08 02:04 UTC) — textarea minHeight 80px → 140px
17. ~~**JSON error catchers**~~ ✅ Done (2026-02-08 03:40 UTC) — registered Rocket catchers for 401, 404, 422, 429, 500 returning JSON instead of HTML

### What's Next (Priority Order)

1. **Verify staging deploy** — Docker build kicked off via `nohup` on 192.168.0.79 (check `/tmp/kanban-build.log` and `docker ps`). Cache mounts were causing stale build failures — removed them. Build runs from scratch (~10-15 min for Rust compile).
2. **Public boards UX** — clarify/extend public board discoverability (per focus notes)
3. **Code cleanup** — review for unused imports, dead code, clippy lints

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

*Last updated: 2026-02-08 03:50 UTC — JSON error catchers added. Docker cache mount fix committed. Staging rebuild in progress (nohup). 44 tests passing.*
