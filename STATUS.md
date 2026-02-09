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

- **Auth refactor** - per-board tokens replacing global API keys
  - `POST /boards` returns `manage_key`, `view_url`, `manage_url`, `api_base`
  - `BoardToken` request guard extracts token from 3 sources (Bearer, X-API-Key, ?key=)
  - Read routes are fully public (just need board UUID)
  - Write routes verify manage_key hash against board
  - Removed: global API keys (/keys CRUD), collaborator system, per-key rate limiting
  - Added: `is_public` flag, `actor_name` fields, `?agent=` on claim
- **Frontend auth integration** - per-board key detection and edit/view modes
  - Detects `?key=` in URL, stores in localStorage per board, cleans URL
  - Edit/View mode badge in header and board view
  - Board creation shows manage URL + view URL + API base with copy buttons
  - Read-only mode hides edit controls (new task button, drag-drop)
  - No global API key required - app loads directly
  - Sidebar shows public boards + direct board ID/URL input
  - `is_public` toggle in board creation modal
- **Column management** - full CRUD for columns from the frontend
  - `PATCH /boards/{id}/columns/{col_id}` - rename, update WIP limit
  - `DELETE /boards/{id}/columns/{col_id}` - delete empty columns (prevents last column deletion)
  - `POST /boards/{id}/columns/reorder` - reorder via ordered ID list
  - Frontend: double-click to inline rename, ⚙️ menu (rename, move left/right, delete), "+" add column button
- **Core API** - all routes working with new auth model
- **Frontend** - React + Vite dashboard with drag-and-drop
- **Docker** - 3-stage multi-stage build
- **Tests** - 44 passing (14 DB/unit integration + 30 HTTP integration), zero clippy warnings
- **Deployed** - kanban.ckbdev.com via Cloudflare Tunnel

### Tech Stack

- Rust 1.83+ / Rocket 0.5 / SQLite (rusqlite)
- React + Vite frontend, unified serving on single port
- CORS: wide open (all origins) - tighten for production

### What's Next (Priority Order)

1. ~~**Deploy updated backend + frontend**~~ ✅ Done (2026-02-07 22:32 UTC)
2. ~~**Comments visible in frontend**~~ ✅ Done (2026-02-07 22:35 UTC) - task detail modal with comments, activity log, add comment form
3. ~~**Identity on actions**~~ ✅ Done (2026-02-07 23:04 UTC) - persistent display name in header, sent with all write ops (create/update/comment/claim)
4. ~~**Task editing in frontend**~~ ✅ Done (2026-02-07 23:04 UTC) - edit button in task detail modal, inline form for title/desc/priority/labels/assignment, delete with confirmation
5. ~~**IP-based rate limiting for board creation**~~ ✅ Done (2026-02-07 23:35 UTC) - ClientIp guard (XFF/X-Real-Ip/socket), 10 boards/hr/IP default, configurable via BOARD_RATE_LIMIT env var, 429 with RATE_LIMIT_EXCEEDED code
6. ~~**Desktop move-to-column in detail modal**~~ ✅ Done (2026-02-07 23:36 UTC) - removed isMobile guard, now available on all screen sizes
7. ~~**Real-time updates via SSE**~~ ✅ Done (2026-02-08 00:06 UTC) - frontend subscribes to `/boards/{id}/events/stream`, debounced 300ms refresh, auto-reconnect with exponential backoff, live connection indicator (green pulsing dot)
8. ~~**Add HTTP integration tests**~~ ✅ Done (2026-02-08 00:06 UTC) - 20 Rocket test client tests covering board CRUD, auth guards (Bearer/X-API-Key/?key=), task CRUD, move/claim/release, comments, archive/unarchive, search, rate limiting
9. ~~**Column management in frontend**~~ ✅ Done (2026-02-08 00:36 UTC) - Backend: PATCH/DELETE/reorder endpoints + 7 tests. Frontend: inline rename, ⚙️ menu, add column button.
10. ~~**Modal positioning updates**~~ ✅ Done (2026-02-08 01:40 UTC) - upper third (8vh top padding) on desktop/tablet, full viewport on mobile
11. ~~**Webhook management in frontend**~~ ✅ Done (2026-02-08 01:45 UTC) - WebhookManagerModal was already built, just needed wiring (render was missing from JSX)
12. ~~**Board settings panel**~~ ✅ Done (2026-02-08 01:50 UTC) - PATCH /boards/{id} endpoint + BoardSettingsModal (name, desc, is_public) + 3 HTTP tests (30 total)
13. ~~**Improved task filtering**~~ ✅ Done (2026-02-08 02:04 UTC) - filter bar with priority, label, and assignee dropdowns; highlights when active; clear button
14. **Auto-fill fields for human-created tasks** - monitoring agent should set priority/labels/assignment based on title+description (Jordan request, 2026-02-08). (This is handled by the Kanban Board Monitor cron job, not the app UI.)
15. ~~**Collapsible sidebar on tablet**~~ ✅ Done (2026-02-08 02:04 UTC) - sidebar collapses on screens < 1024px (was mobile-only at 768px)
16. ~~**Bigger description fields**~~ ✅ Done (2026-02-08 02:04 UTC) - textarea minHeight 80px → 140px
17. ~~**JSON error catchers**~~ ✅ Done (2026-02-08 03:40 UTC) - registered Rocket catchers for 401, 404, 422, 429, 500 returning JSON instead of HTML

### What's Next (Priority Order) - Jordan UI Feedback (2026-02-08)

1. ~~**View/edit mode UX overhaul**~~ ✅ Done (2026-02-08 06:15 UTC) - replaced pill badge with AccessIndicator ("Full Access"/"View Only") + "🔗 Share" button. SharePopover shows copy-able view URL and manage URL (edit-only). Hint for view-only users. Deployed.
2. ~~**Collapsible columns**~~ ✅ Done (2026-02-08 06:34 UTC) - desktop/tablet: click header to collapse to narrow 40px vertical bar with task count + rotated name; click to expand; drag-over auto-expands. Mobile: existing accordion unchanged.
3. ~~**Filter button dark theme fix**~~ ✅ Done (2026-02-08 06:15 UTC) - dark bg with subtle border, blue tint when active.
4. ~~**Filter spacing fix**~~ ✅ Done (2026-02-08 06:15 UTC) - proper padding + dark background on filter row.
5. ~~**Filter button icon**~~ ✅ Done (2026-02-08 06:15 UTC) - ▼/▲ toggle arrow replaces emoji.
5b. ~~**Live indicator simplified**~~ ✅ Done (2026-02-08 06:15 UTC) - dot-only when connected (hover for tooltip), text only on error/reconnecting.
6. ~~**Replace header emoji with real logo**~~ ✅ Done (2026-02-08 07:10 UTC) - SVG kanban board logo in header and welcome screen, replaced all 📋 emoji references.
7. **Remove live indicator** - Jordan confirmed: remove it entirely (2026-02-09). SSE connection can stay for real-time updates but the visible indicator adds noise.
8. ~~**Hamburger menu aesthetics**~~ ✅ Done (2026-02-08 07:10 UTC) - improved border contrast (#475569), lighter text (#cbd5e1), larger padding, rounded corners (6px), smooth transition.
9. ~~**Esc key closes modals**~~ ✅ Done (2026-02-08 08:05 UTC) - useEscapeKey hook on all 5 modals.
10. ~~**Autocomplete/dropdowns for Labels + Assigned To**~~ ✅ Done (2026-02-08 08:32 UTC) - AutocompleteInput component with per-token suggestions for comma-separated labels, arrow keys + Tab/Enter selection, applied to both create and edit modals.
11. ~~**Shift+Enter submits new task**~~ ✅ Done (2026-02-08 08:05 UTC) - works from any field in create task modal.
12. ~~**Fix tiny vertical scroll on desktop**~~ ✅ Done (2026-02-08 08:05 UTC) - app uses height:100vh+overflow:hidden.
13. ~~**Mobile button bar aesthetics**~~ ✅ Done (2026-02-08 09:10 UTC) - + Task button first on mobile, icon buttons grouped compactly.
14. ~~**Sidebar footer aesthetics**~~ ✅ Done (2026-02-08 09:10 UTC) - cleaner spacing, removed "Open by ID" label, accent-colored checkbox.
15. **Public boards UX** - awaiting Jordan's input. Three tiers: private (unlisted), public (listed), manage URL (full access).
16. ~~**Task archiving**~~ ✅ Done (2026-02-09 02:45 UTC) - archived_at column with migration, POST archive/unarchive endpoints, default list hides archived, filter toggle in UI, archive button in task detail modal, 2 new tests (46 total).
17. ~~**Pagination/performance in human UI**~~ ✅ Done (2026-02-09 02:55 UTC) - per-column "Show more" button, displays first 20 tasks with incremental loading in batches of 20.

### What's Next (Priority Order) - New Items (2026-02-09)

1. ~~**Fix mobile collapse logic**~~ ✅ Done (2026-02-09 03:22 UTC) - collapse now waits for tasksLoaded flag before auto-collapsing empty columns, preventing false collapse on initial render.
2. ~~**Share links popout mobile fix**~~ ✅ Done (2026-02-09 03:22 UTC) - share popover uses fixed centering on mobile (<640px) instead of absolute positioning.
3. ~~**Comment auto-scroll**~~ ✅ Already done (verified in code: commentsEndRef.scrollIntoView on comment add).
4. ~~**Full Screen Category View**~~ ✅ Done (2026-02-09 03:36 UTC) - desktop/tablet: ⚙️ menu → "Full Screen" expands column to viewport overlay with responsive multi-column task grid (auto-fill 300px). Esc or click outside to close.
5. **Task workflow states** - Build crons should use In Progress → Review → Done flow instead of Backlog → Done (process improvement).
6. ~~**Move "New Task" button to right side of button bar**~~ ✅ Done (2026-02-09 05:25 UTC) - settings/webhooks icons on left, + Task button on right.
7. ~~**Consolidate access/share buttons**~~ ✅ Done (2026-02-09 05:25 UTC) - removed duplicate AccessIndicator from BoardView, now only in App header.
8. ~~**Modal viewport utilization**~~ ✅ Done (2026-02-09 05:25 UTC) - wider task detail modal (560→680px), taller max (80→90vh), less top padding (8→4vh), comments area expanded to 40vh.
9. **Task archiving UI** - ✅ Done (2026-02-09 02:45 UTC). Board archiving UI still API-only.
10. ~~**Remove live indicator**~~ ✅ Done (2026-02-09 05:25 UTC) - removed LiveIndicator component entirely, SSE still active for real-time sync.
11. ~~**Archived toggle styled as button**~~ ✅ Done (2026-02-09 05:30 UTC) - replaced checkbox with styled toggle button matching filter dropdowns.
12. ~~**Collapse search and filter on mobile**~~ ✅ Done (2026-02-09 18:08 UTC) - search bar and filter row hidden by default on mobile, toggle via 🔍 button in header. Desktop unchanged.
13. ~~**Shift+Enter submits comment**~~ ✅ Done (2026-02-09 18:08 UTC) - onKeyDown handler on comment textarea, same pattern as new task modal.
14. ~~**Clickable access mode indicator**~~ ✅ Done (2026-02-09 18:08 UTC) - "Full Access" / "View Only" button now shows info popover explaining permissions and how to get edit access.
15. ~~**Filter button: swap to funnel/cone SVG icon**~~ ✅ Done (2026-02-09 19:37 UTC) - replaced ▼/▲ caret with classic funnel SVG icon (Lucide-style polygon). Flexbox alignment with gap for icon+text. Commit: (see git log)
16. ~~**Center title in tablet view**~~ ✅ Done (2026-02-09 19:57 UTC) - 3-section header on tablet: hamburger + identity badge (left), centered logo (center), access indicator (right). Desktop layout unchanged.

**HNR Projects Kanban Board (current):**
- Board ID: `1ab5804f-3f1b-4486-b7ae-03cb9616d4c2`
- Manage key: `kb_699c1b40639841cd8aabdea9e7bb7872`
- View URL: https://kanban.ckbdev.com/board/1ab5804f-3f1b-4486-b7ae-03cb9616d4c2
- Manage URL: https://kanban.ckbdev.com/board/1ab5804f-3f1b-4486-b7ae-03cb9616d4c2?key=kb_699c1b40639841cd8aabdea9e7bb7872
- Column IDs: Backlog=`4cfdc374`, Up Next=`e8fd737c`, In Progress=`f3890313`, Review=`338c5c05`, Done=`5518f00d`

### ⚠️ Gotchas

- **Breaking DB change** - new schema has no `api_keys` table. Fresh DB required. Old DBs will not work.
- `cargo` not on PATH by default - use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CORS wide open (all origins) - tighten for production
- **Tests must run with `--test-threads=1`** - tests use `std::env::set_var("DATABASE_PATH", ...)` which races under parallel execution
- Rate limiter now active on board creation (10/hr/IP default, configurable via BOARD_RATE_LIMIT env var)

### Architecture Notes

- `auth.rs` - `BoardToken` request guard extracts token from Bearer/X-API-Key/?key=
- `access.rs` - `require_manage_key()`, `require_board_exists()`, `require_not_archived()`
- `routes.rs` - all write routes take `BoardToken`, hash it, verify against board's `manage_key_hash`
- `db.rs` - `boards` table has `manage_key_hash` and `is_public` columns
- No user/account system - boards are the only resource, tokens are per-board
- Single-threaded SQLite via `Mutex<Connection>`

### Key Product Decisions

- **Pastebin/Excalidraw model** - create board → get management URL, share with others
- **View URL** = read-only, **Manage URL** = full access
- **Unlisted by default** - boards are accessible by UUID but not discoverable unless `is_public=true`
- **actor_name is optional free text** - no identity verification, trust-based
- **Claim vs assignment** preserved - `claimed_by` = actively working, `assigned_to` = responsibility

---

### What's Next (Remaining)

1. ~~**Public boards discovery UX**~~ ✅ Done (2026-02-09 08:06 UTC) — welcome page as discovery hub: hero section, stats bar, card grid of public boards (name/desc/tasks/age), search filter, open-by-ID. Commit: e3f5ca5
2. **Auto-fill fields on new tasks (AI)** - monitoring agent sets priority/labels/assignment based on title+description

### Completed (2026-02-09 Daytime, Session 3 — 07:10 UTC)

- ~~**My Boards sidebar**~~ ✅ Done (07:09 UTC) — sidebar now shows "My Boards" (localStorage-based) instead of public boards. Auto-adds boards when opened. ✏️/👁 icons for edit/view access. ✕ remove button. Public boards in expandable section at bottom. Commit: 8992d62
- ~~**Question: Efficient Updates API Status?**~~ ✅ Answered — activity API with ?since= is implemented (backend + frontend). Kanban monitoring cron doesn't use it yet.
- ~~**Discussion: Rethink Public Boards**~~ ✅ Answered — sidebar = workspace (My Boards), welcome page = discovery. Future: search, categories, featured.
- ~~**Brainstorm: Local Agent Chat**~~ ✅ Answered — recommended simple HTTP pub/sub (Rust/SQLite) with rooms, SSE, mDNS. Awaiting Jordan's direction.
- ~~**Better question tracking**~~ ✅ Process implemented — question/discussion tasks get answered via comment, moved to Review, assigned to Jordan.

### Completed (2026-02-09 Daytime, Session 2 — 06:49 UTC)

- ~~**Share button broken on desktop**~~ ✅ Done (06:55 UTC) - headerRight had overflow:hidden clipping the popover. Commit: 9172e9c
- ~~**Layered escape key handling**~~ ✅ Done (06:55 UTC) - Escape now only closes topmost modal (uses a stack). Fixes issue when task detail + fullscreen both open.
- ~~**Assignee quick-select chips**~~ ✅ Done (06:55 UTC) - green-tinted toggle chips for Assigned To in create + edit modals, matching label chip pattern. Commit: 9172e9c

### Completed (2026-02-09 Daytime, Session 1)

- ~~**Task modal above fullscreen**~~ ✅ Done (06:21 UTC) - z-index 1100 > fullscreen's 1000. Commit: 3d7c5fe
- ~~**Hamburger menu SVG icon**~~ ✅ Done (06:23 UTC) - animated SVG hamburger→X, 34×34px, clean styling. Commit: 4fef1bc
- ~~**Label normalization**~~ ✅ Done (06:26 UTC) - frontend + backend normalize labels to lowercase+dashes. 2 unit tests. Commit: e38f3d1
- ~~**Quick labels by frequency**~~ ✅ Done (06:26 UTC) - chips sorted by most-used. Commit: e38f3d1
- ~~**Activity tracker completeness**~~ ✅ Done (06:29 UTC) - archive/unarchive/delete now logged; move events include column names. Commit: 28a7260
- ~~**Label filter exact match**~~ ✅ Done (06:30 UTC) - was using .includes(), now exact ===. Commit: d2398df
- ~~**Process fix: tasks → Review**~~ ✅ Done - all completed tasks now go to Review (assigned Jordan) instead of straight to Done
- ~~**Board stale task cleanup**~~ ✅ Done - moved 3 already-completed tasks from Backlog to Review
- ~~**Drop-down styling**~~ ✅ Done (05:34 UTC)
- ~~**Code cleanup**~~ ✅ Done (05:34 UTC)
- ~~**Board archiving UI**~~ ✅ Done (06:14 UTC)
- ~~**Remove webhook button**~~ ✅ Done (05:34 UTC)
- ~~**Remove filter bar background**~~ ✅ Done (05:34 UTC)
- ~~**Quick-add label chips**~~ ✅ Done (05:34 UTC)
- ~~**Priority filter fix**~~ ✅ Done (05:55 UTC)
- ~~**Full screen close collapses column**~~ ✅ Done (05:55 UTC)
- ~~**Button/dropdown/toggle height consistency**~~ ✅ Done (05:55 UTC)
- ~~**Unused space at bottom on tablet**~~ ✅ Done (05:55 UTC)
- ~~**Settings button height mismatch**~~ ✅ Done (06:14 UTC)
- ~~**Activity feed / since last visit**~~ ✅ Done (06:14 UTC)
- ~~**Search input height**~~ ✅ Done (06:18 UTC)

### Completed (2026-02-09 Overnight, Session 2 — 08:32 UTC)

- ~~**Comment submit button below bottom of screen on mobile**~~ ✅ Done — modals now use `100dvh` instead of `100vh` on mobile (accounts for browser URL bar/chrome), comments area reduced to 30vh on mobile, explicit bottom padding. Commit: fd71ab2

### Completed (2026-02-09 Overnight, Session 1 — 08:06 UTC)

- ~~**Public boards discovery page**~~ ✅ Done — welcome page as hub: hero with CTA, stats bar (board count + total tasks), card grid with hover effects, search filter, open-by-ID section. Responsive (single column mobile, auto-fill desktop). Commit: e3f5ca5

*Last updated: 2026-02-09 13:25 UTC — fullscreen task click reliability + control sizing consistency. Tests: 53 backend (6 unit + 33 HTTP + 14 integration) all passing.*

### Completed (2026-02-09 Overnight, Session 4 — 09:05 UTC)

- ~~**DB backup automation**~~ ✅ Done — backup script deployed to staging (192.168.0.79). Backs up all 4 SQLite DBs (kanban, qr-service, blog, app-directory) using sqlite3 .backup for WAL-safe copies. Gzip compression. Cron runs every 6 hours. 7-day retention. App directory backed up via docker cp (no sqlite3 in container). Task moved to Review for Jordan.

### Completed (2026-02-09 Overnight, Session 5 — 13:25 UTC)

- **Fullscreen column view: task click reliability** ✅ Done — stopPropagation on TaskCard click + disable drag in fullscreen overlay to avoid touch/tablet drag interference. Commit: 756d403
- **Control height consistency** ✅ Done — add `boxSizing: border-box` to btn/btnSmall/select to make 32px heights visually consistent (border-inclusive). Commit: 756d403
- **Tablet bottom-gap mitigation** ✅ Done — app container uses `100dvh` (dynamic viewport height). Commit: 756d403
