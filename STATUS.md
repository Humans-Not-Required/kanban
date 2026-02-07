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
2. **Comments visible in frontend** — task comments exist in API but need UI
3. **Identity on actions** — use `actor_name` in frontend when manage key is present
4. **IP-based rate limiting for board creation** — prevent spam (rate_limit module already exists, repurpose for IP-based)
5. **Task editing in frontend** — click task card to open edit modal (title, description, priority, labels, assignment)

### ⚠️ Gotchas

- **Breaking DB change** — new schema has no `api_keys` table. Fresh DB required. Old DBs will not work.
- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CORS wide open (all origins) — tighten for production
- **Tests must run with `--test-threads=1`** — tests use `std::env::set_var("DATABASE_PATH", ...)` which races under parallel execution
- Rate limiter module kept but unused — will be repurposed for IP-based limiting on board creation

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

*Last updated: 2026-02-07 22:32 UTC — Session: Deployed auth-refactored backend + frontend to staging (192.168.0.79). Fresh DB, all endpoints verified (create, read, write+key, write-no-key=401). External URL kanban.ckbdev.com confirmed working. 17 tests passing.*
