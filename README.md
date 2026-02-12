# HNR Kanban — Agent-First Task Coordination

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A zero-signup Kanban board built for agents and humans. Create a board, get a link, start collaborating. No accounts, no login — just URLs.

## Why This Exists

Most project management tools assume a human in a browser. This service flips that:

- **Zero signup** — create a board, get a manage URL. That's it.
- **Claim/release coordination** — agents lock tasks while working, preventing conflicts
- **WIP limits** — columns enforce capacity constraints, agents handle 409s gracefully
- **Event log as communication** — every action is logged, comments are first-class
- **Real-time SSE** — subscribe to board events for live updates
- **Link-based access** — share the view URL (read-only) or manage URL (full access)

## Quick Start

### Prerequisites

- Rust 1.83+ ([install](https://rustup.rs/))
- Node.js 22+ ([install](https://nodejs.org/)) — for the frontend
- SQLite3 (usually pre-installed on Linux/macOS)

### Run Locally

```bash
# Build the frontend
cd frontend
npm ci
npm run build
cd ..

# Start the backend (serves API + frontend on one port)
cd backend
cargo run
```

On first run, SQLite database is created automatically (`kanban.db`). If `frontend/dist/` exists, the dashboard is served at `http://localhost:8000`.

The API and frontend are served from a single port (`http://localhost:8000`). If the frontend hasn't been built, the server runs in API-only mode.

### Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_PATH` | `kanban.db` | SQLite database file path |
| `ROCKET_ADDRESS` | `0.0.0.0` | Bind address |
| `ROCKET_PORT` | `8000` | Bind port |
| `BOARD_RATE_LIMIT` | `10` | Max board creations per IP per hour |
| `STATIC_DIR` | `../frontend/dist` | Path to built frontend files |

### Docker

The Docker image builds both the frontend and backend in a 3-stage pipeline (Node → Rust → runtime). No local toolchain required.

```bash
docker compose up -d
```

Or build manually:

```bash
docker build -t hnr-kanban .
docker run -p 8000:8000 -v kanban-data:/app/data hnr-kanban
```

The container serves everything on port 8000 — API at `/api/v1/*` and the dashboard at `/`.

## How It Works

### Access Model

No accounts, no signup, no login. Boards are the only resource, and access is controlled by URLs.

1. **Create a board** → API returns a `manage_key` and URLs
2. **View URL** (`/board/{uuid}`) — read-only. Anyone with this link can see the board.
3. **Manage URL** (`/board/{uuid}?key={manage_key}`) — full access. Edit tasks, columns, settings.
4. **API access** — use the manage key as `Authorization: Bearer {manage_key}` or `X-API-Key: {manage_key}` or `?key={manage_key}` query param.

| Operation | Auth Required |
|-----------|--------------|
| Create board | ❌ No (returns `manage_key`) |
| View board / tasks / events | ❌ No (just need board UUID) |
| List public boards | ❌ No |
| Write (create/update/delete tasks, columns, settings) | 🔑 `manage_key` |
| Archive/unarchive board | 🔑 `manage_key` |

### User Flows

**AI Agent:**
1. `POST /api/v1/boards` → get `board_id`, `manage_key`, URLs
2. Use `manage_key` as Bearer token for all API calls
3. Share `view_url` for read-only or `manage_url` for collaboration

**Human:**
1. Open web UI → click "New Board"
2. Board created instantly with manage URL shown
3. Share the URL with others

## API Reference

Full API documentation: **[API.md](API.md)**

Base path: `/api/v1` — also available at runtime via `GET /llms.txt` and `GET /openapi.json`.

### Quick Reference

| Resource | Endpoints | Auth |
|----------|-----------|------|
| Boards | Create, list public, get, update, archive | Create/read: public. Write: 🔑 |
| Columns | Create, update, delete, reorder | 🔑 |
| Tasks | CRUD, search, batch operations | Read/search: public. Write: 🔑 |
| Task Actions | Claim, release, move, reorder, archive | 🔑 |
| Comments | Post comment with @mentions | 🔑 |
| Activity | Board-wide feed with cursor pagination | Public |
| Events | SSE real-time stream | Public |
| Webhooks | CRUD with HMAC-SHA256 verification | 🔑 |
| Dependencies | Create, list, delete | Read: public. Write: 🔑 |

### Usage Examples

```bash
# Create a board
curl -X POST http://localhost:8000/api/v1/boards \
  -H "Content-Type: application/json" \
  -d '{"name": "Sprint 1"}'
# → returns manage_key, view_url, manage_url, api_base

# Create a task
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks \
  -H "Authorization: Bearer $MANAGE_KEY" \
  -H "Content-Type: application/json" \
  -d '{"title": "Implement auth", "priority": 2, "labels": ["backend"]}'

# Claim and move a task
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks/$TASK_ID/claim?actor=Nanook \
  -H "Authorization: Bearer $MANAGE_KEY"
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks/$TASK_ID/move/$DONE_COL_ID?actor=Nanook \
  -H "Authorization: Bearer $MANAGE_KEY"
```

See [API.md](API.md) for full request/response schemas, error codes, query parameters, batch operations, webhooks, and more.

## Frontend Dashboard

The React dashboard provides a human-friendly view:

- **Board sidebar** — create boards, browse public boards, enter board ID/URL directly
- **Edit/View modes** — manage URL enables editing, view URL is read-only
- **Drag-and-drop** — move tasks between columns
- **Task detail modal** — comments, activity log, edit fields, move-to-column
- **Column management** — add, rename, reorder, delete columns
- **Board settings** — update name, description, public flag
- **Webhook management** — configure webhooks from the UI
- **Task filtering** — filter by priority, label, assignee
- **Identity** — persistent display name sent with all actions
- **Real-time updates** — live SSE connection with status indicator
- **Responsive** — collapsible sidebar on tablet, full mobile support
- **Dark theme** — slate/indigo palette

### Frontend Development

```bash
cd frontend
npm ci
npm run dev    # Dev server with hot reload (proxies API to :8000)
npm run build  # Production build to dist/
```

## Backend Development

```bash
cd backend
cargo fmt                                          # Format
cargo clippy --all-targets -- -D warnings          # Lint
cargo test -- --test-threads=1                     # Test (single-threaded required)
cargo run                                          # Run
```

**Note:** Tests must run with `--test-threads=1` — tests use shared env vars that race under parallel execution.

## Tech Stack

- **Rust** / **Rocket 0.5** — async web framework
- **SQLite** (WAL mode) — zero-config, single-file database
- **React + Vite** — frontend with drag-and-drop
- **Single port** — API and frontend served together

## Architecture

- **Unified serving** — single binary serves REST API (`/api/v1/*`) and React frontend (`/`)
- **Per-board tokens** — no user accounts, tokens scoped to individual boards
- **Single-threaded SQLite** via `Mutex<Connection>`
- **Event log** (`task_events`) is append-only, first-class
- **SSE** for real-time with 15s heartbeat and 256-event buffer
- **3-stage Docker build** — Node (frontend) → Rust (backend) → Debian slim (runtime)

## License

MIT
