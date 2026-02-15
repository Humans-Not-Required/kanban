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

### What's Next (Priority Order) - Current

1. ~~**Mobile UI: Segmented button bar + compact search/filter controls**~~ ✅ Done (2026-02-12 02:02 UTC, updated 02:47 UTC) - On mobile: button bar is now a connected segmented bar at 100% width (Activity | Settings | Search | + Task). +Task button takes exactly 1/3 width (per Jordan direction), other 3 buttons share remaining 2/3 equally. Search button uses arrow icon instead of text. Filter button uses funnel icon only (no text). Commits: e917f57, 985b8b7.
2. ~~**Custom dropdown chevrons (StyledSelect)**~~ ✅ Done (2026-02-12 04:55 UTC) - Created reusable `StyledSelect` component with custom SVG downward chevron. Uses `appearance:none`/`-webkit-appearance:none` to hide native browser chrome. Chevron positioned at `right:10px` with `paddingRight:32px`. Replaced all 9 `<select>` elements app-wide. Fixes: chevron too close to right edge on desktop, missing on iOS Safari. Commit: 0916f79.
3. ~~**New board default columns**~~ ✅ Done (2026-02-09) - Remove columns field from board creation UI. Boards are created with default columns: Backlog, Up Next, In Progress, Review, Done. Keep API field as optional.
4. ~~**Replace priority dropdown with 4-way button toggle**~~ ✅ Done (2026-02-13 04:05 UTC) - Created reusable `PriorityToggle` component: 4 connected buttons (Critical/High/Medium/Low) with color-coded backgrounds (red/orange/yellow/green). Active button filled, inactive muted. Replaced `<StyledSelect>` in both CreateTaskModal and TaskDetailModal edit mode. Filter bar dropdown unchanged. Commit: ac4eec1. 56 tests passing (42 HTTP + 14 integration).
5. ~~**Mobile scroll: header + board controls can scroll out of view**~~ ✅ Done (2026-02-13 12:03 UTC) - On mobile, switch to page scroll (no fixed-height internal scroller) so header, board title, button bar, search bar, and filters can scroll away. Commit: d82babd. 56 tests passing (42 HTTP + 14 integration).
6. ~~**Mobile: display name control visible**~~ ✅ Done (2026-02-13 12:23 UTC) - Show IdentityBadge on mobile (compact) header so users can set/change their display name. Commit: f4a2370.
7. ~~**Mobile: when search is expanded, always expand filters (hide filter toggle)**~~ ✅ Done (2026-02-13 14:19 UTC) - On mobile, the Search toggle now always shows filters and the in-search Filter toggle button is hidden. Search toggle highlights when filters are active. Commit: f447d13.
8. ~~**Mobile: filter controls fill each row**~~ ✅ Done (2026-02-13 14:52 UTC) - On mobile, filters now use a grid so Priority + Label each take 50% width; Assignee takes 50%; Archived + Clear share the remaining space. Clear button label is shortened on mobile. Commit: 4af6254.
9. ~~**Mobile: column move menu uses up/down labels**~~ ✅ Done (2026-02-13 15:03 UTC) - Column options menu now shows "Move Up"/"Move Down" on mobile (instead of left/right) since columns stack vertically. Commit: a699bd9.
10. ~~**SSE Live indicator: desktop pill tag**~~ ✅ Done (2026-02-13 22:00 UTC) - Desktop: pill tag with "LIVE" text, green translucent bg, rounded border, pulsing dot. Positioned left of IdentityBadge. Mobile: compact 7px dot. Commit: 699a094. 56 tests passing.

### What's Next (Priority Order) - Jordan UI Feedback (2026-02-08)

1. ~~**View/edit mode UX overhaul**~~ ✅ Done (2026-02-08 06:15 UTC) - replaced pill badge with AccessIndicator ("Full Access"/"View Only") + "🔗 Share" button. SharePopover shows copy-able view URL and manage URL (edit-only). Hint for view-only users. Deployed.
2. ~~**Collapsible columns**~~ ✅ Done (2026-02-08 06:34 UTC) - desktop/tablet: click header to collapse to narrow 40px vertical bar with task count + rotated name; click to expand; drag-over auto-expands. Mobile: existing accordion unchanged.
3. ~~**Filter button dark theme fix**~~ ✅ Done (2026-02-08 06:15 UTC) - dark bg with subtle border, blue tint when active.
4. ~~**Filter spacing fix**~~ ✅ Done (2026-02-08 06:15 UTC) - proper padding + dark background on filter row.
5. ~~**Filter button icon**~~ ✅ Done (2026-02-08 06:15 UTC) - ▼/▲ toggle arrow replaces emoji.
5b. ~~**Live indicator simplified**~~ ✅ Done (2026-02-08 06:15 UTC) - dot-only when connected (hover for tooltip), text only on error/reconnecting.
6. ~~**Replace header emoji with real logo**~~ ✅ Done (2026-02-08 07:10 UTC) - SVG kanban board logo in header and welcome screen, replaced all 📋 emoji references.
7. ~~**Move live indicator to header**~~ ✅ Done (2026-02-11 06:30 UTC) - Moved from floating bottom-left to inline 7px dot in App header. Commit: 5501622.
8. ~~**Ctrl/Cmd+Enter for submit**~~ ✅ Done (2026-02-11 03:30 UTC) - Changed Shift+Enter to Ctrl+Enter (Win/Linux) / Cmd+Enter (macOS) for new task modal and comment submission. Commit: 09a3faa.
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

17. ~~**Enrich activity endpoint for created and comment events**~~ ✅ Done (2026-02-09 22:50 UTC) - `created` and `comment` events now include full `task` snapshot. `comment` events also include `recent_comments` (last 10, newest first). `moved`/`archived`/`updated` stay lean. Batch-fetched for efficiency. llms.txt updated. Test extended. Commit: cdb2ecc
18. ~~**Fix clipped popover on share/full-access buttons (desktop)**~~ ✅ Done (2026-02-11 03:10 UTC) - Header had `overflow: hidden` which clipped the absolutely-positioned popovers. Changed to `overflow: visible`.
19. ~~**Prevent accidental modal dismiss when form has content**~~ ✅ Done (2026-02-11 03:58 UTC) - Backdrop click and Esc key now only dismiss when no unsaved content. Applies to CreateTaskModal, CreateBoardModal, TaskDetailModal (editing/comment), BoardSettingsModal (changed fields). Cancel/Create buttons still always work. Commit: 484b123.
20. ~~**Remove horizontal rules around search/filters**~~ ✅ Done (2026-02-11 05:55 UTC) - Removed `borderBottom` from `boardHeader` style and from the filter row div. Cleaner visual flow between header → search → filters → columns.
21. ~~**Remove indicator circles from search/filter buttons**~~ ✅ Done (2026-02-11 18:25 UTC) - Removed ● indicator from search toggle, Search button, and Filter button. Active state now uses indigo background (#312e81) + border (#6366f1) + light text (#a5b4fc) instead of dot indicators.
22. ~~**iOS: Prevent page zoom when focusing search field on mobile**~~ ✅ Done (2026-02-11 18:56 UTC) - iOS Safari auto-zooms on form inputs with font-size < 16px. Fixed 5 inputs: board search field, discovery page search field, and 3 filter selects (priority, label, assignee) - all changed from 0.78-0.8rem to 16px. Also fixed search button border: indigo highlight now only shows when search results are active, not when bar is merely open. Commits: 3bda80a, 23813a3. 55 tests passing.

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
- **Tests are parallel-safe** — each test uses `init_db_with_path()` with a unique temp DB (no env var race)
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

1. ~~**Public boards discovery UX**~~ ✅ Done (2026-02-09 08:06 UTC) - welcome page as discovery hub: hero section, stats bar, card grid of public boards (name/desc/tasks/age), search filter, open-by-ID. Commit: e3f5ca5
2. **Auto-fill fields on new tasks (AI)** - monitoring agent sets priority/labels/assignment based on title+description
3. **Request Edit Access flow** (future) - view-only users request edit access → owner notification → approve/deny. Safety: snapshots/undo before granting.
4. ~~**Change submit hotkey from Shift+Enter to Ctrl/Cmd+Enter**~~ ✅ Done (2026-02-11 03:30 UTC) - Commit: 09a3faa.
5. ~~**Stale task filter (updated_before param)**~~ ✅ Done (2026-02-11 06:10 UTC) - `?updated_before=ISO-8601` on GET /tasks filters by updated_at < timestamp. Enables stale task detection crons. 1 new test (40 total HTTP). Commit: 9b44919.
6. **Any new Jordan feedback** - all 2026-02-10 items completed, awaiting review.

### Jordan Feedback (2026-02-11 18:12 UTC)

- ~~**Search button border issue**~~ ✅ Fixed (2026-02-11 18:56 UTC) - Search toggle showed indigo border when bar was merely open (`showSearchBar`). Changed to only highlight when `searchResults !== null`. Also fixed "Search" action button which had `border: 'none'` when inactive (now inherits standard btnSmall border). Commit: 23813a3.

### Completed (2026-02-11 Daytime, Session 17 - 22:30 UTC)

- **Fix: allow saving edited task with description but no title** ✅ Done - Save button's `disabled` prop checked only `!editTitle.trim()`, ignoring description. Changed to `(!editTitle.trim() && !editDesc.trim())` to match `saveEdit()` validation and backend logic. Backend was already correct. Commit: 57c81ab. 55 tests passing.

### Completed (2026-02-11 Daytime, Session 16 - 22:03 UTC)

- **Remove by-name header from task details** ✅ Done - Removed the `by {task.created_by}` line from task detail metadata in App.jsx. Cleaner task detail view. Commit: d83606a. 55 tests passing.

### Completed (2026-02-11 Daytime, Session 15 - 21:33 UTC)

- **Search button white outline** ✅ Fixed - Search button's `btnSmall` style had `border: 1px solid #475569` which appeared as a white outline vs the borderless toolbar buttons (⚙️ 📊). Changed both Search and Filter buttons to `border: none` when inactive, keeping indigo border when active (search results / active filters). Commit: 4ea23d7. 55 tests passing.

### Completed (2026-02-11 Daytime, Session 14 - 07:58 UTC)

- **Search field highlight** ✅ Done - When search results are active (`searchResults !== null`), the search input gets an indigo border, dark indigo background (`#1e1b4b`), and subtle box-shadow glow. Search button text turns indigo. Mobile search toggle button shows a dot indicator (●) when search is active. Clear visual signal that results are being filtered. Commit: 7587ce8. 54 tests passing.

### Completed (2026-02-11 Overnight, Session 2 - 10:10 UTC)

- **Fix Anonymous in activity log (reorder + batch)** ✅ Done - Three remaining sources of anonymous/unattributed activity entries fixed: (1) `reorder_task` endpoint now accepts `?actor=` query param (was hardcoded "anonymous"). (2) Batch operations now accept `"actor"` field in request JSON (was hardcoded "batch" for move/update). (3) `batch_delete` now logs deletion events in activity feed (was entirely missing). All three enforce `require_display_name` when board setting is enabled. 1 new HTTP test. Commit: e8063d4. 55 tests passing (41 HTTP + 14 integration).

### Completed (2026-02-11 Overnight, Session 1 - 08:45 UTC)

- **Filter dropdown left indentation alignment** ✅ Done - Filter row had hardcoded `padding: '8px 16px'` while search bar, board header, and columns container all use 20px (desktop) / 12px (mobile) horizontal padding. Changed filter row to use `isMobile`-aware padding (`8px 20px` desktop, `8px 12px` mobile) for consistent alignment. Commit: fa1fb7e. 54 tests passing.

### Completed (2026-02-11 Daytime, Session 13 - 07:48 UTC)

- **Some activities erroneously showing Anonymous** ✅ Done - Issue #3 fixed: task detail activity log was missing `eventLabel` cases for `archived`, `unarchived`, and `deleted` event types. They fell through to `default: return evt.event_type` (raw lowercase, no icon). Added 📦 Archived, 📤 Unarchived, 🗑️ Deleted. Issues #1/#2 (Anonymous on actions) were already resolved by commit d9ba12e (frontend passes `?actor=` on all write endpoints). Commit: eed7724. 54 tests passing.

### Completed (2026-02-11 Daytime, Session 12 - 06:45 UTC)

- **Activity Box My Items** ✅ Done - Made "My Items" the left-most tab and default when the activity panel opens. Tab order is now: My Items → All Recent → Since Last Visit. Default tab changed from 'since'/'all' to 'mine'. Commit: 233f2e1. 54 tests passing.

### Completed (2026-02-11 Daytime, Session 10 - 06:30 UTC)

- **Move SSE live indicator to header** ✅ Done - Relocated LiveIndicator from floating bottom-left position (inside BoardView) to inline in App header (headerRight area, before AccessIndicator). 7px dot, green pulse when connected, red + "Reconnecting…" text when disconnected. SSE status lifted to App via `onSseStatusChange` callback. Status resets when navigating away from a board. Commit: 5501622. 54 tests passing.

### Completed (2026-02-11 Daytime, Session 9 - 06:20 UTC)

- **URGENT: Display name errors blocking many actions - UI/API out of sync** ✅ Done - Root cause: frontend `deleteTask`, `archiveTask`, `unarchiveTask`, `moveTask`, and `releaseTask` never sent the user's display name. Backend expects `?actor=` query param on these endpoints; without it, defaults to "anonymous", which fails `require_display_name` check. Fix: all 5 endpoints now include `?actor={displayName}` from localStorage. Commit: d9ba12e. 54 tests passing (40 HTTP + 14 integration).

### Completed (2026-02-11 Daytime, Session 8 - 05:50 UTC)

- **Bug: Anonymous actions bypass require_display_name** ✅ Done - Audited all write endpoints. Previously only task creation and comment creation checked `require_display_name`. Added the check to 7 more endpoints: update_task, delete_task, archive_task, unarchive_task, claim_task, release_task, move_task. New comprehensive test covers all affected endpoints. Commit: 179c495. 53 tests passing (39 HTTP + 14 integration).

### Completed (2026-02-11 Daytime, Session 7 - 05:22 UTC)

- **Filter button text color fix** ✅ Done - Filter button had black text when no filters active because `color: undefined` override removed btnSmall's `#cbd5e1`. Fixed: explicit `#cbd5e1` when inactive, `#a5b4fc` highlight when active. Border also restored to match btnSmall default. Commit: b9de811. 52 tests passing.

### Completed (2026-02-11 Daytime, Session 6 - 05:12 UTC)

- **Square X close buttons on share/mode popovers** ✅ Done - Replaced "Dismiss" text links at the bottom of SharePopover and AccessIndicator mode info popover with square X close buttons (24×24px) in the top right corner, consistent with other modals (btnClose style). Webhook secret dismiss also updated to "Close" button. Commit: fed926a. 52 tests passing.

### Completed (2026-02-11 Daytime, Session 5 - 04:53 UTC)

- **Shift+Enter submits comment** ✅ Done - Added Shift+Enter as additional submit hotkey for both comment textarea and new task modal (alongside existing Ctrl/Cmd+Enter). Commit: d087b86. 52 tests passing.

### Completed (2026-02-11 Daytime, Session 4 - 04:10 UTC)

- **Live SSE streaming indicator** ✅ Done - Floating bottom-left dot: green pulsing dot when connected (tooltip-only, no text), red dot + "Reconnecting…" text when disconnected. Positioned out of the toolbar to avoid the clutter that caused the original removal. Uses `ssePulse` keyframe for subtle breathing animation. Re-enabled `sseStatus` state + callback. Commit: 86f3793. 52 tests passing.

### Completed (2026-02-11 Daytime, Session 3 - 03:30 UTC)

- **Submit hotkey: Ctrl/Cmd+Enter** ✅ Done - Changed Shift+Enter to Ctrl+Enter (Win/Linux) / Cmd+Enter (macOS) for new task modal global handler and comment textarea. Standard convention (Gmail, Slack, etc). Commit: 09a3faa. 52 tests passing.

### Completed (2026-02-11 Daytime, Session 2 - 01:10 UTC)

- **Filter Fields** ✅ Done - Replaced label and assignee chip buttons in filter bar with `<select>` dropdowns matching priority field style. All three filter fields now consistent dropdowns (priority, label, assignee). Commit: cc335c8. 52 tests passing.

### Completed (2026-02-11 Daytime, Session 1 - 00:55 UTC)

- **Title / Description Requirement** ✅ Done - Title is now optional. Either title or description must be provided (not necessarily both). Backend: `title` field uses `deserialize_string_or_null` (defaults to empty string), validation changed from `EMPTY_TITLE` to `EMPTY_TASK` (requires at least one non-empty). Update route also validates to prevent clearing both. Frontend: TaskCard shows truncated description (60 chars) when no title, task detail header shows description preview (80 chars) in muted italic, My Items tab shows description fallback. Create/edit modals updated - submit enabled when either field has content. 3 new test cases (52 total: 38 HTTP + 14 integration). Commit: 0f1b6d4.

### Completed (2026-02-10 Overnight, Session 4 - 08:07 UTC)

- **Activity panel overhaul: two tabs** ✅ Done - "Recent" tab (activity feed with since-last-visit toggle) and "My Items" tab (assigned tasks grouped by column + user's own activity). Click tasks in My Items to open detail modal. Tab badges show unread/assignment counts. Responsive. Commit: 79b4070. Kanban task: 34b6a40a → Review.
- **Check deploy** ✅ Verified - CI passing, Watchtower pulling, health checks OK. Task: d6c982ea → Review.
- **Tasks skip Review process fix** ✅ Confirmed working - task: 88f214a9 → Review.

### Completed (2026-02-10 Daytime, Session 3 - 07:40 UTC)

- **Monotonic seq cursor pagination on activity endpoint** ✅ Done - `seq INTEGER` column on task_events table with migration + backfill. `?after=<seq>` cursor param on GET /boards/{id}/activity. Seq assigned via next_event_seq() on event insert. Response includes `seq` field. `after=` returns ASC order for feed consumption. `since=` preserved for backward compat. 50 tests passing (36 HTTP + 14 integration). Commit: f6fc0eb. Playbooks updated: kanban-monitor.md + agent-chat-monitor.md now use `?after=` instead of `?since=`.

### Completed (2026-02-10 Daytime, Session 2 - 07:04 UTC)

- **Sidebar: My Boards only** ✅ Done - Removed Public Boards expandable list and Archived Boards toggle from sidebar. Added "Browse Public Boards" link that navigates to welcome/discovery page. Sidebar is now My Boards only. Cleaned up dead state vars. Commit: eaec899
- **Webhook button → Board Settings** ✅ Done - Moved webhook management into Board Settings modal. Edit-mode users see "⚡ Manage Webhooks" button that opens the webhook manager. Commit: b4f13e2
- **Require display name setting** ✅ Done - New `require_display_name` boolean on boards. When enabled, task creation and commenting reject empty/anonymous actor names (DISPLAY_NAME_REQUIRED error). Toggle in Board Settings. DB migration auto-adds column. 1 new HTTP test (62 total). Commit: e39f671
- **Deploy pipeline verified** ✅ - CI passing, Watchtower pulling images, health checks OK.

### Completed (2026-02-10 Daytime, Session 1 - 06:07 UTC)

- **Edit box problems** ✅ Done - Two fixes: (1) Edit textarea now starts at 140px (was 60px) and auto-grows with content as user types, fixing the too-small edit box on iPhone SE. (2) `selectedTask` now syncs with refreshed tasks data via useEffect, so task detail view updates immediately after save without close/reopen. Commit: 1a10aec

### Completed (2026-02-09 Daytime, Session 7 - 21:57 UTC)

- **Timestamp timezone fix** ✅ Done - Added `parseUTC()` helper that normalizes API timestamps (space-separated, no TZ marker) to ISO 8601 with 'Z' suffix. Applied to `formatTime`, `formatTimeAgo`, `due_at`, board `created_at`, and activity feed comparisons. All timestamps now correctly display in the user's local timezone. Commit: cc7b9c0
- **View mode button unlock** ✅ Done - "View Only" access indicator now shows a manage key input field. Users can paste a key, it gets validated server-side via no-op PATCH, and if valid the UI instantly upgrades to Full Access mode. Invalid keys show error. Added `api.validateKey()`. Commit: ca652dd
- **My Boards task cleanup** - moved already-completed "My Boards / Public Boards" task to Review.

### Completed (2026-02-09 Daytime, Session 6 - 21:34 UTC)

- **Square close buttons** ✅ Done - dedicated `btnClose` style (32×32px) applied to task detail, board settings, activity, and webhooks modal close buttons. Standardized × character. Commit: 219fdb5
- **Actor attribution on API endpoints** ✅ Done - added optional `?actor=` query param to move_task, archive_task, unarchive_task, delete_task, and release_task endpoints. Build crons can now properly attribute actions with `?actor=Nanook`. Backward compatible (defaults to "anonymous"). Commit: e140d90

### Completed (2026-02-09 Daytime, Session 5 - 21:35 UTC)

- **Quick-reassign button** ✅ Done - amber ↩ button in task detail header. Board settings: target column dropdown + assignee input. Moves task to configured column and optionally reassigns. Backend: `quick_reassign_column_id` + `quick_reassign_to` columns with column validation. 1 new HTTP test (49 total). Commit: 539daa7

### Completed (2026-02-09 Daytime, Session 4 - 21:20 UTC)

- **Simplified board creation** ✅ Done - removed columns field from UI, backend defaults to 5 columns (Backlog, Up Next, In Progress, Review, Done). API still accepts custom columns. Commit: cb31635
- **Quick-done button** ✅ Done - green ✓ button in task detail header, moves to configurable column (default: last). Board settings: target column dropdown + auto-archive toggle. Column validation in backend. 1 new HTTP test (54 total). Commit: 801d433
- **Board housekeeping** - moved 2 already-completed tasks (mobile search collapse, share button fix) to Review.

### Completed (2026-02-09 Daytime, Session 3 - 07:10 UTC)

- ~~**My Boards sidebar**~~ ✅ Done (07:09 UTC) - sidebar now shows "My Boards" (localStorage-based) instead of public boards. Auto-adds boards when opened. ✏️/👁 icons for edit/view access. ✕ remove button. Public boards in expandable section at bottom. Commit: 8992d62
- ~~**Question: Efficient Updates API Status?**~~ ✅ Answered - activity API with ?since= is implemented (backend + frontend). Kanban monitoring cron doesn't use it yet.
- ~~**Discussion: Rethink Public Boards**~~ ✅ Answered - sidebar = workspace (My Boards), welcome page = discovery. Future: search, categories, featured.
- ~~**Brainstorm: Local Agent Chat**~~ ✅ Answered - recommended simple HTTP pub/sub (Rust/SQLite) with rooms, SSE, mDNS. Awaiting Jordan's direction.
- ~~**Better question tracking**~~ ✅ Process implemented - question/discussion tasks get answered via comment, moved to Review, assigned to Jordan.

### Completed (2026-02-09 Daytime, Session 2 - 06:49 UTC)

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

### Completed (2026-02-09 Overnight, Session 2 - 08:32 UTC)

- ~~**Comment submit button below bottom of screen on mobile**~~ ✅ Done - modals now use `100dvh` instead of `100vh` on mobile (accounts for browser URL bar/chrome), comments area reduced to 30vh on mobile, explicit bottom padding. Commit: fd71ab2

### Completed (2026-02-09 Overnight, Session 1 - 08:06 UTC)

- ~~**Public boards discovery page**~~ ✅ Done - welcome page as hub: hero with CTA, stats bar (board count + total tasks), card grid with hover effects, search filter, open-by-ID section. Responsive (single column mobile, auto-fill desktop). Commit: e3f5ca5

### Completed (2026-02-10 Overnight, Session 1 - 08:17 UTC)

- **@mention support in comments** ✅ Done - Backend: `extract_mentions()` parses `@Name` and `@"Quoted Name"` from comment text. Mentions stored in comment data JSON (no migration needed). Activity endpoint: `?mentioned=<name>` filter. `BoardActivityItem` includes top-level `mentions` field on comment events. Frontend: @mentions highlighted purple (gold for self-mentions). My Items tab uses structured mentions for reliable filtering. 2 new HTTP tests (52 total: 38 HTTP + 14 integration). Commit: be4de71
- **Board housekeeping** - moved "Enrich activity endpoint" and "DB backup automation" tasks from Up Next to Review (already completed).

### Completed (2026-02-10 Daytime, Session 5 - 17:35 UTC)

- **Button color consistency (take 2)** ✅ Done - Root cause: search toggle (mobile) and filter button were using translucent `#3b82f622`/`#3b82f633` backgrounds when active, making them appear lighter than the solid `#334155` on 📊/⚙️ buttons. Fix: removed all translucent background overrides; active state now indicated by solid indigo border (`#6366f1`) only. All toolbar buttons now have identical dark background. Commit: 92ce8ec. Task: 7164b9d5 → Review.

### Completed (2026-02-10 Overnight, Session 3 - 08:50 UTC)

- **Button color consistency** ⚠️ Partial - `btnSmall` background changed from transparent to `#334155` (matching secondary buttons). Border `#475569`, text `#cbd5e1`. Filter button inherits from base instead of separate overrides. All toolbar buttons now visually consistent. Commit: a68dad7. Kanban task: 7164b9d5 → Review.
- **Mobile task detail button layout** ✅ Done - 2-row layout on mobile: row 1 = title + close button (max space for title), row 2 = action buttons right-aligned (↩ reassign, ✓ done, 📦 archive, ✏️ edit). Reassign button now visible on mobile. Desktop layout unchanged. Commit: a68dad7. Kanban task: 27bdec0f → Review.
- **Task sweep** ✅ Done - reviewed all non-archived tasks. Moved "Rethink Public Boards" to Review (already addressed). 5 backlog items remain (new projects + ideas). 59 tasks in Review awaiting Jordan. No stuck or missed items. Kanban task: 7fad1f80 → Review.

### Completed (2026-02-10 Overnight, Session 2 - 08:37 UTC)

- **Consistent chip-style selectors everywhere** ✅ Done - Filter bar: replaced `<select>` dropdowns for label and assignee with clickable chip buttons (blue for labels, green for assignees). Edit task modal: added missing label chips. All three locations (create, edit, filter) now use identical chip styling with consistent colors. Priority filter kept as `<select>` (fixed set of values). Overflow indicator (+N) for large sets. Commit: 20a7be3

### Completed (2026-02-10 Overnight, Session 4 - 09:52 UTC)

- **Verification pass** - all Jordan (2026-02-10) feature requests confirmed implemented and deployed:
  - ✅ Board option to disable anonymous (require_display_name) - backend + frontend + tests
  - ✅ @mention support (user tagging) - extraction, storage, filtering, highlighting
  - ✅ Activity panel overhaul (two tabs: Recent + My Items) - implemented
  - ✅ Consistent chip-style selectors (labels + assignees everywhere) - implemented
  - ✅ Monotonic seq cursor pagination on activity endpoint - implemented
  - ✅ Public boards: sidebar is "My Boards" only + "Browse Public Boards" link to discovery page
  - ✅ Deploy pipeline healthy: Watchtower pulling latest images, CI/CD all green, all 4 services UP
- **Kanban tasks updated** - added verification comments to "Board option to disable anonymous" (c10bc7dc) and "Check deploy" (d6c982ea)

*Last updated: 2026-02-10 09:52 UTC - verification pass. Tests: 52 backend (38 HTTP + 14 integration) all passing.*

### Completed (2026-02-09 Overnight, Session 4 - 09:05 UTC)

- ~~**DB backup automation**~~ ✅ Done - backup script deployed to staging (192.168.0.79). Backs up all 4 SQLite DBs (kanban, qr-service, blog, app-directory) using sqlite3 .backup for WAL-safe copies. Gzip compression. Cron runs every 6 hours. 7-day retention. App directory backed up via docker cp (no sqlite3 in container). Task moved to Review for Jordan.

### Completed (2026-02-09 Overnight, Session 5 - 13:25 UTC)

- **Fullscreen column view: task click reliability** ✅ Done - stopPropagation on TaskCard click + disable drag in fullscreen overlay to avoid touch/tablet drag interference. Commit: 756d403
- **Control height consistency** ✅ Done - add `boxSizing: border-box` to btn/btnSmall/select to make 32px heights visually consistent (border-inclusive). Commit: 756d403
- **Tablet bottom-gap mitigation** ✅ Done - app container uses `100dvh` (dynamic viewport height). Commit: 756d403

---

## New Requests / Direction (2026-02-10)

### Public boards: UX + safety decisions
- Public boards should be **view-only by default** (don't grant edit to strangers in v1 - avoids "someone trashed my board").
- Remove any "Public Boards" list/section from the **sidebar** entirely.
  - Sidebar should be **My Boards** only.
  - Add a **"Browse Public Boards"** link that goes to a dedicated discovery page (welcome/discovery hub).
- Future (needs design + implementation): **Request Edit Access** flow
  - View-only users click "Request Edit Access" → owner gets a notification → approve/deny.
  - Safety features if we ever allow it: snapshots/backups before granting edit, audit log, undo/revert.

### New feature requests
- **Board option to disable anonymous** task creation + commenting (require display name).
- **User tagging** in comments/text fields.
- **Activity box overhaul**: two tabs/modes
  1) Recent events (last 24h)
  2) Items assigned to current user + comments that @mention them (ties into tagging)
- **Consistency sweep**: labels + assigned-to inputs should use the same quick-select chips + autocomplete everywhere.

### Ops
- **Check deploy**: Jordan reports some issues marked fixed / moved to Review aren't visible live - verify deploy pipeline + whether staging is stale.

*Last updated: 2026-02-11 23:33 UTC*

### Completed (2026-02-11 Daytime, Session 11 - 06:40 UTC)

- **Activity box: Replace since-last-visit toggle with third tab** ✅ Done - Activity panel now has 3 tabs: "All Recent" (last 50 events), "Since Last Visit" (events since last visit, with badge count), "My Items" (unchanged). Since Last Visit tab only appears when user has a previous visit recorded. Default tab is Since Last Visit when available, All Recent otherwise. Removed the toggle button from inside the Recent tab. Commit: 0afd975. 54 tests passing (40 HTTP + 14 integration).

### Completed (2026-02-12 Daytime, Session 25 - 01:15 UTC)

- **Activity button icon doesn't match theme** ✅ Done - Replaced 📊 emoji with inline SVG pulse/activity icon (Lucide-style `<polyline>` heartbeat line). Matches the existing filter button SVG style and dark theme (`stroke="currentColor"` inherits button text color). Updated both the toolbar button (14×14) and Activity panel header (16×16). Frontend builds clean, 56 backend tests passing.

### Completed (2026-02-12 Daytime, Session 23 - 00:35 UTC)

- **Create API.md documentation** ✅ Done - Comprehensive API.md (500+ lines) documenting all endpoints: boards, columns, tasks, task actions, batch operations, comments, activity feed, SSE, webhooks, dependencies. Includes request/response schemas, error code table (20 codes), query parameters, object reference, auth model, WIP limits, display name enforcement. Slimmed README from inline API details to quick reference table + examples linking to API.md. Commit: 38647d4. 55 tests passing (41 HTTP + 14 integration).

### Completed (2026-02-12 Daytime, Session 22 - 00:15 UTC)

- **Fix actor param inconsistency** ✅ Done - Two naming inconsistencies fixed: (1) Claim endpoint `?agent=` → `?actor=` to match all other query param endpoints (move, archive, delete, release, reorder). (2) Batch endpoint `actor` → `actor_name` in JSON body to match create/update request models. Also fixed claim event data key from `agent` to `actor`. Frontend, tests, llms.txt updated. Commit: 6f59db0. 55 tests passing (41 HTTP + 14 integration).

### Completed (2026-02-12 Daytime, Session 21 - 00:03 UTC)

- **Board settings: enlarge description box on desktop** ✅ Done - Description textarea in BoardSettingsModal was using `styles.input` with `minHeight: '60px'` (tiny). Changed to `styles.textarea` which has `minHeight: '140px'`, proper `resize: 'vertical'`, and `fontFamily: 'inherit'`. Consistent with other textareas. Commit: 88e29bd. 55 tests passing (41 HTTP + 14 integration).

### Completed (2026-02-11 Daytime, Session 20 - 23:33 UTC)

- **Activity view: tab count badges hidden when tab is selected** ✅ Done - Tab badges for "My Items" (task count) and "Since Last Visit" (new event count) had conditions `tab !== 'mine'` / `tab !== 'since'` that hid them on the active tab. Removed active-tab guard so badges are always visible. Commit: a6a8264. 55 tests passing (41 HTTP + 14 integration).

### Completed (2026-02-11 Daytime, Session 19 - 23:25 UTC)

- **Make modals full-screen on mobile (edit mode, share)** ✅ Done - SharePopover and AccessIndicator mode info popup now expand to full viewport on mobile instead of small centered floating boxes. Full-screen overlay with larger text, bigger touch targets for copy/unlock buttons, 16px font on inputs to prevent iOS zoom. Desktop behavior unchanged. Commit: 2cc098b. 55 tests passing (41 HTTP + 14 integration).

### Completed (2026-02-11 Daytime - 22:43 UTC)

- **Fix full-access modal dismiss behavior** ✅ Done - Added invisible backdrop overlay behind the Full Access/View Only mode info popup so clicking outside closes it (same pattern as SharePopover). Previously required the × close button. Commit: 7c65736. 55 tests passing (41 unit + 14 integration).

### Completed (2026-02-11 Daytime - 23:47 UTC)

- **Filter button white background on desktop** ✅ Fixed - When `hasActiveFilters` was false, `background: undefined` removed the inline style, causing browser default white background on `<button>`. Changed to explicit `'#334155'` (matching btnSmall base). Commit: 619c3ba. 55 tests passing (41 HTTP + 14 integration).

### Completed (2026-02-12 Overnight, Session - 09:07 UTC)

- **Search/Filter buttons: remove inactive border highlight** ✅ Done - Use `border: 1px solid transparent` when inactive (prevents layout shift) and suppress browser focus ring outlines. Commit: 953a824. 56 tests passing (42 HTTP + 14 integration).

### Completed (2026-02-12 Daytime, Session 26 - 04:44 UTC)

- **Fix iPad zoom issue on kanban board** ✅ Done - Three-part fix: (1) Fixed 3 remaining inputs with font-size < 16px that triggered Safari auto-zoom on focus: column rename (0.85rem→16px), display name (0.8rem→16px), add column (0.85rem→16px). (2) Added `visibilitychange` handler to reset viewport zoom when returning from another app by temporarily forcing `maximum-scale=1`. (3) On iOS devices, set `maximum-scale=1` in viewport meta tag - prevents auto-zoom on input focus but does NOT block user pinch-to-zoom (iOS 10+). Also added `touch-action: manipulation` (prevents double-tap zoom) and `-webkit-text-size-adjust: 100%` (prevents text inflation on orientation change) to body CSS. Commit: a3d0eea. 56 tests passing (42 HTTP + 14 integration).

### Jordan Direction (2026-02-13 06:30 UTC)

5. **"New board default" text still visible** (task 828e8cb8) - Jordan: "I still see this text: 'Boards are created with default columns: Backlog, Up Next, In Progress, Review, Done.' That needs to be removed." This was previously marked done (2026-02-09, commit cb31635) but the text persists somewhere in the UI. Need to find and remove it.

### Jordan Direction (2026-02-12 01:17-01:28 UTC)

1. **Actor param fix → check playbooks** (task 5edb6c0c) - Jordan: "Check through all playbooks to make sure that this change is reflected if it is relevant." The `?agent=` → `?actor=` and `actor` → `actor_name` changes (commit 6f59db0) need to be verified across all playbooks that call kanban API.
2. ~~**Shift+Enter still working**~~ ✅ Fixed (2026-02-12 04:18 UTC) - Removed `e.shiftKey` from both new task modal global handler and comment textarea onKeyDown. Now ONLY Ctrl/Cmd+Enter submits. Commit: 4445ea2. 56 tests passing.
3. **Drop-down chevron not visible** (task 169fd738) - Jordan: "I don't see these changes being reflected. On desktop the chevron is too close to the right side and on mobile there is no chevron at all. Research the new iOS look to see if there is anything there that could be affecting this." Custom chevron may not be rendering on iOS. Need to research iOS Safari select element styling.
   - **Update (2026-02-13 17:10 UTC):** Jordan still reports chevron spacing/missing chevron; verify deploy/caching + iOS rendering.
4. **SKILL.md research pivot** (task 55faf1e0, non-kanban) - Jordan: "I got the details wrong. Sky.ai uses SKILL.md. Research SKILL.md usage." Original task was STATE.md vs llms.txt; now pivoted to SKILL.md as the standard used by Sky.ai.

## Incoming Directions (Work Queue)

**Triage checks (2026-02-13):**
- ✅ Fix: allow saving task with description but no title - verified already shipped in `main` (commit 57c81ab). Safe to close.
- ✅ Fix full-access modal dismiss behavior (click outside to close) - verified already shipped in `main` (commit 7c65736). Safe to close.
- (Repeat ping via NATS directions) Jordan/anonymous re-pinged on prior kanban items (My Boards/Public Boards, webhook button placement, DB backup automation verification). All are already captured below as completed/verified.

<!-- WORK_QUEUE_DIRECTIONS_START -->
- [x] Search button styling (3 tasks: 7164b9d5, 8be9ae71, 834c296e) - Fixed via commit eec6adf: outline:none, :focus-visible suppression, appearance:none CSS reset. Needs hard refresh.
- [x] Board manager: Up Next as triage gate (4a3f787a) - Already implemented: Up Next is Tier 1 (highest priority) in board-manager playbook. No changes needed.
- [x] Activity button icon (f37e7f86) - Fixed: SVG pulse/heartbeat icon (2026-02-12 session 25).
- [x] My Links page title (7c121df8) - Changed to "HNR Platform - Control Panel". Deployed.
- [x] My Links ordering (da8e5dca) - Reordered by usage: Chat > Kanban > Dashboard > Watchpost > Blog > Apps > QR > Docs > Website.
- [x] My Links 2 columns (a8a8dc72) - Already implemented (CSS grid: 1fr 1fr on desktop).
- [x] Filter button white bg (5ab4b5e7) - Fixed in prior commits (619c3ba, 624428c).
- [x] Board settings desc box (467a1a0b) - Fixed: minHeight 140px (commit 88e29bd).
- [x] API.md documentation (c5a99ef0) - Created 500+ lines (commit 38647d4).
- [x] Sibling naming feedback (c4b4586d, ff0c06f8, 1057e69d) - Collected: consensus is "Dispatch". Forge+Lux voted Dispatch, Drift agreed.
- [x] iOS dropdown chevron (169fd738) - **Re-fixed (commit 8cb5764):** Replaced overlay approach with background-image SVG data URL. Previous fix still failed on iOS because absolute-positioned elements get hidden by native select compositing. New approach embeds chevron as backgroundImage, converts `background` shorthand to `backgroundColor` to prevent wipe, removes wrapper div entirely.
- [x] Filter left indentation (2b82ac9c, 0a8a402f) - Fixed (commit fa1fb7e).
- [x] Activity My Items default tab (00d6fe2a) - Verified done (commit 233f2e1).
- [x] Anonymous in activity log (6a7d5297) - Verified done (commits eed7724, d9ba12e, e8063d4, 179c495).
- [x] Search field highlight (88cd8c73) - Verified done (commit 7587ce8).
- [x] Remove horizontal rules (25e8aa27) - Verified done.
- [x] Remove by-name header (89ea4c9d) - Verified done (commit d83606a).
- [x] Hotkeys Ctrl/Cmd+Enter (34c80cee) - Verified done (commit 4445ea2).
- [x] Activity endpoint enrichment (4eddb19c) - Verified: created events get task snapshot, comment events get task + recent_comments.
- [ ] Kanban: Find a catchier product name (cf7a6d06) - Sibling consensus: "Dispatch". Awaiting Jordan's decision.
- [x] "New board default" text still visible (828e8cb8) - Confirmed: only a JSX comment (`{/* ... */}`) remains in code, invisible to users. No visible text in any page. Browser cache issue.
<!-- WORK_QUEUE_DIRECTIONS_END -->

### Completed (2026-02-14 Overnight, Session — 10:45 UTC)

- **Browser button focus artifact suppression** ✅ Done — Added `outline: none` to `btn()` style, `:focus-visible` to CSS reset rules, `appearance: none` on all buttons. Should eliminate any remaining browser-default borders/outlines that Jordan keeps reporting. Commit: eec6adf. 56 tests passing.
- **iOS dropdown chevron fix** ✅ Done — Added `backgroundImage: 'none'` to StyledSelect to suppress iOS native chevron. CSS-level `appearance: none` + `background-image: none` on select elements. Adjusted chevron spacing (right: 10px→12px, paddingRight: 32px→36px). Commit: c9af77f. 56 tests passing.
- **My Links page improvements** ✅ Done — Changed title from "HNR Staging — Admin Links" to "HNR Platform — Control Panel". Reordered services by usage priority. Deployed to staging.
- **Batch triage verification** ✅ Done — Verified 15+ direction items as already completed. Collected sibling naming feedback (consensus: Dispatch). Reported all via NATS. Cleaned up STATUS.md directions section.

### Completed (2026-02-14 Daytime, Session — 05:35 UTC)

- **Fix search results not updating after Done+Archive** ✅ Done - Root cause: `searchResults` was a stale snapshot that never refreshed after task actions. When `baseTasks = searchResults ?? tasks` and the user did Done+Archive, only `tasks` was refreshed via `loadTasks()`, but `searchResults` kept the old data. Fix: created `refreshAll()` that refreshes both tasks AND re-runs the active search query. Replaced `loadTasks` with `refreshAll` as `onRefresh` for all child components (Column, FullScreenColumnView, TaskDetailModal, CreateTaskModal). SSE handler also refreshes search via ref-based state to avoid subscription churn. Commit: 2757bfa. 56 tests passing (42 HTTP + 14 integration).
- **Fix remaining search refresh timing bug** ✅ Done - All task action handlers (Done+Archive, archive toggle, delete, move, reassign) now `await onRefresh()` before calling `onClose()`. Previously `onRefresh()` was fire-and-forget, causing a race condition where the modal closed before the search refresh API call completed, leaving stale results visible. Commit: b66f97a.

### Completed (2026-02-14 Daytime, Session - 01:52 UTC)

- **Fix inconsistent spacing in button bar + header** ✅ Done - (1) Desktop button bar: removed inner grouping div so Activity, Settings, and +Task buttons all share uniform 8px gap (was 4px between Activity/Settings, 8px before +Task). (2) AccessIndicator: removed 4px gap between mode and share pill buttons (they form a connected shape). Commit: acca46d.
- **Search label dropdown alphabetical** ✅ Done - Filter label dropdown now sorted alphabetically via `allLabelsSorted`. Chip suggestions in create/edit modals remain frequency-sorted. Commit: acca46d. 56 tests passing (42 HTTP + 14 integration).

### Completed (2026-02-13 Daytime, Session - 22:14 UTC)

- **Column settings popout dismiss on outside click** ✅ Done - Added mousedown/touchstart outside-click handler using useRef + useEffect on the column settings ⚙️ menu. Clicking anywhere outside the popout now closes it. Works on desktop and mobile (touch). Commit: 4f5a764. 56 tests passing (42 HTTP + 14 integration).

### Completed (2026-02-13 Daytime, Session - 21:32 UTC)

- **Mobile filter grid layout fix** ✅ Done - Root cause: `StyledSelect` wraps `<select>` in a wrapper `<div>` but only forwarded `flex`/`minWidth`/`marginBottom` to the wrapper - not `gridColumn`. On mobile, the CSS grid uses `repeat(4, 1fr)` with `span 2` per item for 50% width, but `gridColumn` never reached the wrapper divs, so each dropdown defaulted to `span 1` = 25%. Fix: forward `gridColumn` and `width` from style prop to wrapper, clear `gridColumn` on inner select. Also bumped Archive button font-size to 16px on mobile (prevents iOS auto-zoom) and shortened label to "Archive". Layout now: Row 1: Priority(50%) + Label(50%), Row 2: Assignee(50%) + Created By(50%), Row 3: Archive(50%) + Clear(50%). Commit: 90806ed. 56 tests passing (42 HTTP + 14 integration).

### Completed (2026-02-13 Daytime, Session - 21:14 UTC)

- **Add Created By filter** ✅ Done - New "Created By" dropdown in filter bar. Collects unique creator names from task data dynamically. Filter logic matches on `task.created_by`. Mobile grid layout updated: Archived and Clear buttons now `gridColumn: span 2` (full row) since 5th filter pushes them to row 3. Commit: 00e01dd. 56 tests passing (42 HTTP + 14 integration).

### Completed (2026-02-13 Daytime, Session - 20:53 UTC)

- **Disable Clear Filters button instead of hiding** ✅ Done - Clear Filters button is now always visible in the filter bar but disabled (opacity 0.4, cursor not-allowed) when no filters are active. Previously it was conditionally rendered (hidden). Archived button mobile grid column simplified to always span 1. Commit: 644723c. 56 tests passing.

### Completed (2026-02-13 Daytime, Session - 20:47 UTC)

- **Activity view priority sorting + tab restructure** ✅ Done - (1) My Items tab now has "By Priority" / "By Column" toggle (defaults to priority). Priority sort shows flat list with P0 at top, column name shown inline. (2) "My Recent Activity" moved from within My Items into its own separate tab. (3) "Since Last Visit" tab removed. (4) Tab order: My Items → My Recent Activity → All Recent. Extracted `renderTaskItem` helper for DRY rendering. Commit: 8cf5132. 56 tests passing (42 HTTP + 14 integration).

### Completed (2026-02-13 Daytime, Session - 20:17 UTC)

- **Search/Filter button style mismatch (multiple reports)** ✅ Done - Root cause: Search and Filter buttons used `btnSmall` style (border: 1px solid, color: #cbd5e1) while toolbar buttons (Activity, Settings) used `btn('secondary')` (border: none, color: #fff). Changed both Search and Filter to use `btn('secondary')` as base style. All toolbar-area buttons now render with identical background/border/color when inactive. Commit: 624428c. 56 tests passing (42 HTTP + 14 integration).

### Completed (2026-02-13 Daytime, Session - 19:28 UTC)

- **Remove number from activity indicator button** ✅ Done - Replaced numbered badge (14-16px circle with count) with simple 8px indigo dot on both mobile and desktop activity buttons. Indicator still appears when new activity exists, just without the number. Commit: 7c75e32. 56 tests passing (42 HTTP + 14 integration).

### Completed (2026-02-13 Overnight)

- **Board settings: Save button on right** ✅ Done - Reordered the Board Settings modal action row so "Save Changes" is right-aligned (moved after archive controls + `marginLeft: auto`). Commit: bc4c1bb.
- **New Task modal: priority toggle height + mobile compact labels** ✅ Done - Removed extra bottom margin from the column dropdown in the New Task modal so the Priority segmented control matches its height. On mobile, priority buttons now show color dots instead of full text labels. Commit: 4aca0ad.
- **Share/View popovers: close button size consistency** ✅ Done - SharePopover and access mode info popover now use standard `btnClose` (32×32) like other modals. Commit: 2dc67a9.

### Completed (2026-02-14 Overnight, Session 2 — 12:30 UTC)

- **Fix popover clipping on tablet** ✅ Done — Root cause: app container had `overflow: hidden` which clipped absolutely-positioned popovers from the header. Removed overflow constraint from app (main area has its own overflow handling). Commit: ae1fa83. 56 tests passing.
- **Simplify Activity panel** ✅ Done — Removed "My Recent Activity" tab per Jordan feedback. Activity panel now has 2 tabs: "My Items" and "All Activity". Cleaner, less data fetching. Commit: ae1fa83.
- **Shareable task links** ✅ Done — URL now includes `?task={id}` when task detail is open. Navigate to board URL with `?task=` to auto-open that task. 🔗 Copy Link button in task detail (desktop + mobile). Manage keys stripped from shared links. Commit: 509f928.
- **Search toggle visual feedback** ✅ Done — Mobile search button now shows slightly lighter background (#475569) when search bar is expanded. Commit: 509f928.
- **Verified playbooks for actor param** ✅ Confirmed — All playbooks use `?actor=` and `"actor_name"` correctly (board-manager.md verified).

### Completed (2026-02-14 Daytime, Session — 19:55 UTC)

- **StyledSelect chevron: background-image approach** ✅ Done — Previous overlay approach (absolute-positioned SVG span) failed on iOS Safari because native `<select>` compositing layers hide overlaid elements. Callers passing `background` shorthand also wiped `backgroundImage: 'none'`. New approach: embed chevron as SVG data URL in `backgroundImage`, destructure `background` → `backgroundColor`, remove wrapper div entirely. More spacing (14px from right, 40px padding). Commit: 8cb5764. 64 tests passing (50 HTTP + 14 integration).
- **8 new HTTP tests** ✅ Done — Coverage for previously untested endpoints: OpenAPI JSON, llms.txt, single task GET (+ 404), task events history, column creation (+ no-auth), dependency deletion. Commit: 23c332d. Test count: 56 → 64.

## Incoming directions (2026-02-13T17:49:01Z)
- ~~Jordan reports popovers (Share + Access/View mode) are still clipped on tablet.~~ ✅ Fixed (ae1fa83)
- ~~Jordan: 🔍 mobile toggle button background color should match other header buttons.~~ ✅ Fixed (509f928)
- ~~Jordan: Activity/since-last-visit feature not useful; rethink or simplify.~~ ✅ Simplified (ae1fa83)
- (refs: task c3e8b248, 52695b91, 035f9168, 1f4f04be)
