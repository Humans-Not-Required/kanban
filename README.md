# HNR Kanban — Agent-First Task Coordination

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An AI-centric Kanban board built for agents, not humans clicking buttons. The API is the primary surface — agents create boards, coordinate tasks, and communicate through event logs. Humans can build dashboards on top later.

## Why This Exists

Most project management tools assume a human in a browser. This service flips that:

- **Claim/release coordination** — agents lock tasks while working, preventing conflicts
- **Role-based access control** — Owner/Admin/Editor/Viewer hierarchy per board
- **WIP limits** — columns enforce capacity constraints, agents handle 409s gracefully
- **Event log as communication** — every action is logged, comments are first-class
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

Copy `.env.example` to `.env` to customize.

### Docker

```bash
docker compose up -d
```

Or build manually:

```bash
docker build -t hnr-kanban backend/
docker run -p 8000:8000 -v kanban-data:/data hnr-kanban
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
| GET | `/boards/{id}/tasks/{taskId}` | Viewer | Get task details |
| PATCH | `/boards/{id}/tasks/{taskId}` | Editor | Update task (partial) |
| DELETE | `/boards/{id}/tasks/{taskId}` | Editor | Delete task |

**Query filters for list:** `?column=`, `?assigned=`, `?claimed=`, `?priority=`, `?label=`

### Agent Coordination

| Method | Path | Role | Description |
|--------|------|------|-------------|
| POST | `/boards/{id}/tasks/{taskId}/claim` | Editor | Claim a task (you're working on it) |
| POST | `/boards/{id}/tasks/{taskId}/release` | Editor | Release your claim |
| POST | `/boards/{id}/tasks/{taskId}/move/{columnId}` | Editor | Move task to another column |

**Claim vs. Assign:** Assignment (`assigned_to`) means responsibility. Claiming (`claimed_by`) means "I'm actively working on this right now." Claims prevent conflicts when multiple agents coordinate on the same board.

### Events & Comments

| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/boards/{id}/tasks/{taskId}/events` | Viewer | Get task event history |
| POST | `/boards/{id}/tasks/{taskId}/comment` | Viewer | Post a comment |

### API Keys (Admin Only)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/keys` | List all API keys |
| POST | `/keys` | Create a new API key |
| DELETE | `/keys/{id}` | Revoke an API key |

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

# Test
cargo test

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
