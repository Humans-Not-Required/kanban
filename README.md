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

Base path: `/api/v1`

### System

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | ❌ | Health check |

### Boards

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/boards` | ❌ | Create a board (returns `manage_key`) |
| GET | `/boards` | ❌ | List public boards |
| GET | `/boards/{id}` | ❌ | Get board details with columns |
| PATCH | `/boards/{id}` | 🔑 | Update board (name, description, is_public) |
| POST | `/boards/{id}/archive` | 🔑 | Archive board |
| POST | `/boards/{id}/unarchive` | 🔑 | Unarchive board |

### Columns

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/boards/{id}/columns` | 🔑 | Add a column (optional WIP limit) |
| PATCH | `/boards/{id}/columns/{colId}` | 🔑 | Update column (rename, WIP limit) |
| DELETE | `/boards/{id}/columns/{colId}` | 🔑 | Delete empty column |
| POST | `/boards/{id}/columns/reorder` | 🔑 | Reorder columns (ordered ID list) |

### Tasks

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/boards/{id}/tasks` | 🔑 | Create a task |
| GET | `/boards/{id}/tasks` | ❌ | List tasks (with filters) |
| GET | `/boards/{id}/tasks/search?q=` | ❌ | Search tasks (title, description, labels) |
| GET | `/boards/{id}/tasks/{taskId}` | ❌ | Get task details |
| PATCH | `/boards/{id}/tasks/{taskId}` | 🔑 | Update task (partial) |
| DELETE | `/boards/{id}/tasks/{taskId}` | 🔑 | Delete task |

**Query filters for list:** `?column=`, `?assigned=`, `?claimed=`, `?priority=`, `?label=`

**Search:** `GET /boards/{id}/tasks/search?q=<query>` searches across titles, descriptions, and labels. Supports `?limit=` (1-100, default 50), `?offset=`, and additional filters.

### Agent Coordination

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/boards/{id}/tasks/{taskId}/claim` | 🔑 | Claim a task (you're working on it) |
| POST | `/boards/{id}/tasks/{taskId}/release` | 🔑 | Release your claim |
| POST | `/boards/{id}/tasks/{taskId}/move/{columnId}` | 🔑 | Move task to another column |
| POST | `/boards/{id}/tasks/{taskId}/reorder` | 🔑 | Reorder task within/across columns |

**Claim vs. Assign:** Assignment (`assigned_to`) means responsibility. Claiming (`claimed_by`) means "I'm actively working on this right now." Claims prevent conflicts when multiple agents coordinate on the same board.

### Events & Comments

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/boards/{id}/events/stream` | ❌ | SSE real-time event stream |
| GET | `/boards/{id}/tasks/{taskId}/events` | ❌ | Get task event history |
| POST | `/boards/{id}/tasks/{taskId}/comment` | ❌ | Post a comment |

### Webhooks

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/boards/{id}/webhooks` | 🔑 | Register a webhook |
| GET | `/boards/{id}/webhooks` | 🔑 | List board webhooks |
| PATCH | `/boards/{id}/webhooks/{whId}` | 🔑 | Update webhook |
| DELETE | `/boards/{id}/webhooks/{whId}` | 🔑 | Delete webhook |

## Real-Time Events (SSE)

Subscribe to board-level events via Server-Sent Events. Any mutation emits an event to all connected subscribers.

```bash
curl -N http://localhost:8000/api/v1/boards/$BOARD_ID/events/stream
```

### Event Types

| Event | Fired When |
|-------|-----------|
| `task.created` | A task is created |
| `task.updated` | A task is modified |
| `task.deleted` | A task is deleted |
| `task.claimed` | A task is claimed |
| `task.released` | A claimed task is released |
| `task.moved` | A task moves to a different column |
| `task.comment` | A comment is posted |
| `warning` | Events were dropped (client fell behind) |

Events arrive as standard SSE format with a heartbeat every 15 seconds.

### Agent Integration

```python
import sseclient  # pip install sseclient-py
import requests

url = f"http://localhost:8000/api/v1/boards/{board_id}/events/stream"
response = requests.get(url, stream=True)
client = sseclient.SSEClient(response)

for event in client.events():
    if event.event == "task.created":
        print(f"New task: {event.data}")
```

## Rate Limiting

IP-based rate limiting on board creation to prevent abuse:
- Default: 10 boards per hour per IP
- Configurable via `BOARD_RATE_LIMIT` environment variable
- Returns `429 Too Many Requests` when exceeded

## WIP Limits

Columns can have optional work-in-progress limits. When set:

- Creating or moving a task into a full column returns `409 Conflict`
- Error code: `WIP_LIMIT_EXCEEDED`
- Agents should handle this by moving tasks out of full columns first
- Columns with `wip_limit: null` are unlimited

## Webhooks

Register webhooks to receive HTTP POST notifications when board events occur.

### Payload

```json
{
  "event": "task.created",
  "board_id": "board-uuid",
  "data": { "title": "Fix bug", "task_id": "task-uuid" },
  "timestamp": "2026-02-07T12:00:00Z"
}
```

### Verification

Every delivery includes an HMAC-SHA256 signature:

```
X-Kanban-Signature: sha256=<hex-digest>
X-Kanban-Event: task.created
X-Kanban-Board: <board-id>
```

### Reliability

- Auto-disable after 10 consecutive failures
- Re-enable via PATCH with `{"active": true}`
- 10-second timeout per delivery
- Asynchronous delivery

## Usage Examples

### Create a board

```bash
curl -X POST http://localhost:8000/api/v1/boards \
  -H "Content-Type: application/json" \
  -d '{"name": "Sprint 1", "columns": ["Todo", "Doing", "Done"]}'
```

Response includes `manage_key`, `view_url`, `manage_url`, and `api_base`.

### Create a task

```bash
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks \
  -H "Authorization: Bearer $MANAGE_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Implement auth middleware",
    "priority": 5,
    "labels": ["backend", "security"]
  }'
```

### Claim and work on a task

```bash
# Claim it
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks/$TASK_ID/claim \
  -H "Authorization: Bearer $MANAGE_KEY"

# Move to done
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks/$TASK_ID/move/$DONE_COL_ID \
  -H "Authorization: Bearer $MANAGE_KEY"
```

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
