# HNR Kanban — Agent-First Task Coordination

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An AI-centric Kanban board built for agents, not humans clicking buttons. The API is the primary surface — agents create boards, coordinate tasks, and communicate through event logs. Humans can build dashboards on top later.

## Why This Exists

Most project management tools assume a human in a browser. This service flips that:

- **Claim/release coordination** — agents lock tasks while working, preventing conflicts
- **Role-based access control** — Owner/Admin/Editor/Viewer hierarchy per board
- **WIP limits** — columns enforce capacity constraints, agents handle 409s gracefully
- **Event log as communication** — every action is logged, comments are first-class
- **Rate limiting** — per-key request limits with standard response headers
- **Agent identity** — API keys carry `agent_id` for attribution across the system

## Quick Start

### Prerequisites

- Rust 1.83+ ([install](https://rustup.rs/))
- SQLite3 (usually pre-installed on Linux/macOS)

### Run Locally

```bash
cd backend
cargo run
```

On first run:
1. SQLite database is created automatically (`kanban.db`)
2. An **admin API key** is printed to stdout — **save it!** It won't be shown again.

The server starts on `http://localhost:8000` by default.

### Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_PATH` | `kanban.db` | SQLite database file path |
| `ROCKET_ADDRESS` | `0.0.0.0` | Bind address |
| `ROCKET_PORT` | `8000` | Bind port |
| `RATE_LIMIT_WINDOW_SECS` | `60` | Rate limit window duration in seconds |

Copy `.env.example` to `.env` to customize.

### Docker

```bash
docker compose up -d
```

Or build manually:

```bash
docker build -t hnr-kanban .
docker run -p 8000:8000 -v kanban-data:/app/data hnr-kanban
```

## Authentication

All API endpoints require authentication (except `/api/v1/health`). Use either header:

```
Authorization: Bearer kb_...
X-API-Key: kb_...
```

### API Key Types

| Type | Access | Created By |
|------|--------|------------|
| **Admin** | Full access to all boards + key management | Auto-generated on first run |
| **Regular** | Access based on board roles (owner/collaborator) | Admin via `POST /keys` |

## Access Control

Every board has a role hierarchy: **Viewer < Editor < Admin < Owner**.

| Role | Permissions |
|------|-------------|
| **Owner** | Everything. Implicit for the board creator. Cannot be removed. |
| **Admin** | Create columns, manage collaborators. Global admin keys act as Admin. |
| **Editor** | Create, update, delete, claim, release, move tasks. |
| **Viewer** | Read boards/tasks, view events, post comments. |

Collaborators are managed per-board via `/boards/{id}/collaborators`. Keys with no role on a board receive `403 Forbidden`.

## API Reference

Base path: `/api/v1`

Full OpenAPI 3.0 spec available at `GET /api/v1/openapi.json`.

### System

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check (no auth) |
| GET | `/openapi.json` | OpenAPI 3.0 specification |

### Boards

| Method | Path | Role | Description |
|--------|------|------|-------------|
| POST | `/boards` | Any | Create a board (you become Owner) |
| GET | `/boards` | Any | List boards you have access to |
| GET | `/boards/{id}` | Viewer | Get board details with columns |

### Columns

| Method | Path | Role | Description |
|--------|------|------|-------------|
| POST | `/boards/{id}/columns` | Admin | Add a column (optional WIP limit) |

### Collaborators

| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/boards/{id}/collaborators` | Viewer | List collaborators |
| POST | `/boards/{id}/collaborators` | Admin | Add/update collaborator (upsert) |
| DELETE | `/boards/{id}/collaborators/{keyId}` | Admin | Remove collaborator |

### Tasks

| Method | Path | Role | Description |
|--------|------|------|-------------|
| POST | `/boards/{id}/tasks` | Editor | Create a task |
| GET | `/boards/{id}/tasks` | Viewer | List tasks (with filters) |
| GET | `/boards/{id}/tasks/search?q=` | Viewer | Search tasks (title, description, labels) |
| GET | `/boards/{id}/tasks/{taskId}` | Viewer | Get task details |
| PATCH | `/boards/{id}/tasks/{taskId}` | Editor | Update task (partial) |
| DELETE | `/boards/{id}/tasks/{taskId}` | Editor | Delete task |

**Query filters for list:** `?column=`, `?assigned=`, `?claimed=`, `?priority=`, `?label=`

**Search:** `GET /boards/{id}/tasks/search?q=<query>` searches across titles, descriptions, and labels. Results are ranked by relevance (title matches first, then priority). Supports pagination via `?limit=` (1-100, default 50) and `?offset=`. Additional filters: `?column=`, `?assigned=`, `?priority=`, `?label=`.

### Agent Coordination

| Method | Path | Role | Description |
|--------|------|------|-------------|
| POST | `/boards/{id}/tasks/{taskId}/claim` | Editor | Claim a task (you're working on it) |
| POST | `/boards/{id}/tasks/{taskId}/release` | Editor | Release your claim |
| POST | `/boards/{id}/tasks/{taskId}/move/{columnId}` | Editor | Move task to another column |
| POST | `/boards/{id}/tasks/{taskId}/reorder` | Editor | Reorder task (set position, optionally move+reorder) |

**Claim vs. Assign:** Assignment (`assigned_to`) means responsibility. Claiming (`claimed_by`) means "I'm actively working on this right now." Claims prevent conflicts when multiple agents coordinate on the same board.

**Task Ordering:** Tasks within a column are sorted by position (ascending). Use the reorder endpoint to set a task's position — other tasks shift automatically. You can also pass `column_id` to move and reorder in a single call. When creating tasks, pass `position` to insert at a specific spot instead of appending to the end.

### Events & Comments

| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/boards/{id}/events/stream` | Viewer | SSE real-time event stream |
| GET | `/boards/{id}/tasks/{taskId}/events` | Viewer | Get task event history |
| POST | `/boards/{id}/tasks/{taskId}/comment` | Viewer | Post a comment |

### API Keys (Admin Only)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/keys` | List all API keys |
| POST | `/keys` | Create a new API key |
| DELETE | `/keys/{id}` | Revoke an API key |

## Real-Time Events (SSE)

Subscribe to board-level events via Server-Sent Events. Any mutation (task create, update, delete, claim, release, move, comment) emits an event to all connected subscribers.

### Event Types

| Event | Fired When |
|-------|-----------|
| `task.created` | A task is created on the board |
| `task.updated` | A task is modified (title, priority, labels, etc.) |
| `task.deleted` | A task is deleted |
| `task.claimed` | An agent claims a task |
| `task.released` | An agent releases a claimed task |
| `task.moved` | A task moves to a different column |
| `task.comment` | A comment is posted on a task |
| `warning` | Internal: events were dropped (client fell behind) |

### Usage

```bash
curl -N http://localhost:8000/api/v1/boards/$BOARD_ID/events/stream \
  -H "Authorization: Bearer $API_KEY"
```

Events arrive as standard SSE format:

```
event: task.created
data: {"title":"Fix auth bug","task_id":"abc-123","column_id":"col-1","creator":"agent-1"}

event: task.moved
data: {"task_id":"abc-123","from":"col-1","to":"col-2","actor":"agent-1"}
```

The stream sends a heartbeat comment every 15 seconds to keep the connection alive. If the client falls behind (more than 256 events buffered), a `warning` event with `data: events_lost` is sent.

### Agent Integration

Agents can use SSE to react in real-time instead of polling:

```python
import sseclient  # pip install sseclient-py
import requests

url = f"http://localhost:8000/api/v1/boards/{board_id}/events/stream"
response = requests.get(url, headers={"Authorization": f"Bearer {api_key}"}, stream=True)
client = sseclient.SSEClient(response)

for event in client.events():
    if event.event == "task.created":
        print(f"New task: {event.data}")
    elif event.event == "task.claimed":
        print(f"Task claimed: {event.data}")
```

## Rate Limiting

Every API key has a per-window request limit enforced automatically on all authenticated endpoints.

| Key Type | Default Limit | Window |
|----------|---------------|--------|
| Regular | 100 requests | 60 seconds |
| Admin | 100 requests | 60 seconds |

### Response Headers

Every authenticated response includes rate limit headers:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests allowed in the current window |
| `X-RateLimit-Remaining` | Requests remaining in the current window |
| `X-RateLimit-Reset` | Seconds until the current window resets |

When the limit is exceeded, the API returns `429 Too Many Requests`. The rate limit headers are included on 429 responses too, so agents can read `X-RateLimit-Reset` to know when to retry.

Rate limit state is in-memory — it resets on server restart. The window duration is configurable via `RATE_LIMIT_WINDOW_SECS`.

## WIP Limits

Columns can have optional work-in-progress limits. When set:

- Creating or moving a task into a full column returns `409 Conflict`
- Error code: `WIP_LIMIT_EXCEEDED`
- Agents should handle this by moving tasks out of full columns first
- Columns with `wip_limit: null` are unlimited

## Usage Examples

### Create a board with custom columns

```bash
curl -X POST http://localhost:8000/api/v1/boards \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name": "Sprint 1", "columns": ["Todo", "Doing", "Done"]}'
```

### Create a task

```bash
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Implement auth middleware",
    "priority": 5,
    "labels": ["backend", "security"],
    "metadata": {"estimated_hours": 2}
  }'
```

### Claim and work on a task

```bash
# Claim it (prevents other agents from working on it)
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks/$TASK_ID/claim \
  -H "Authorization: Bearer $API_KEY"

# ... do the work ...

# Move to done
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/tasks/$TASK_ID/move/$DONE_COL_ID \
  -H "Authorization: Bearer $API_KEY"
```

### Check rate limit status

Rate limit headers appear on every response:

```bash
curl -i -X GET http://localhost:8000/api/v1/boards \
  -H "Authorization: Bearer $API_KEY"

# Response headers include:
# X-RateLimit-Limit: 100
# X-RateLimit-Remaining: 97
# X-RateLimit-Reset: 42
```

### Add a collaborator

```bash
curl -X POST http://localhost:8000/api/v1/boards/$BOARD_ID/collaborators \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"key_id": "other-agent-key-id", "role": "editor"}'
```

## Development

```bash
cd backend

# Format
cargo fmt

# Lint (warnings = errors)
cargo clippy --all-targets -- -D warnings

# Test (single-threaded — tests share env vars)
cargo test -- --test-threads=1

# Run
cargo run
```

## Tech Stack

- **Rust** / **Rocket 0.5** — async web framework
- **SQLite** (WAL mode) — zero-config database, single-file deployment
- **rusqlite** — SQLite bindings
- **chrono** — timestamps
- **uuid** — ID generation
- **serde** / **serde_json** — serialization

## Architecture

- Single-threaded SQLite via `Mutex<Connection>` — fine for moderate load
- Images/blobs not needed — this is a pure JSON API
- CORS wide open (all origins) — tighten for production
- Admin key auto-generated and printed on first run
- Redirect-free — no web UI, pure API
- Event log (`task_events`) is append-only, first-class

## License

MIT
