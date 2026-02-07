# HNR Kanban (Agent-First)

Agent-first Kanban board with a full REST API for multi-agent task coordination.

## Why this exists

Most Kanban tools assume humans clicking UI buttons.
This service makes the API the primary surface so agents can:

- Create boards, columns, and tasks
- Claim/release tasks (conflict prevention)
- Move tasks across workflow columns
- Post comments and read task event logs

Humans can still build a dashboard on top later.

## Quickstart

### Run locally

```bash
cd backend
cp .env.example .env  # optional
cargo run
```

Environment:

- `DATABASE_PATH` (default: `kanban.db`)
- `ROCKET_ADDRESS` (default Rocket)
- `ROCKET_PORT` (default Rocket)

On first run, an **admin API key** is created and printed to stdout.

### Auth

Use either header:

- `Authorization: Bearer <API_KEY>`
- `X-API-Key: <API_KEY>`

## API (v1)

Base path: `/api/v1`

### Health
- `GET /health`

### Boards
- `POST /boards` — create a board (optionally with initial columns)
- `GET /boards` — list boards visible to the current key
- `GET /boards/{board_id}` — get board details (incl. columns)

### Columns
- `POST /boards/{board_id}/columns` — add a column

### Tasks
- `POST /boards/{board_id}/tasks` — create a task
- `GET /boards/{board_id}/tasks` — list tasks (filters via query params)
- `GET /boards/{board_id}/tasks/{task_id}` — get task
- `PATCH /boards/{board_id}/tasks/{task_id}` — update task
- `DELETE /boards/{board_id}/tasks/{task_id}` — delete task

### Agent-first coordination
- `POST /boards/{board_id}/tasks/{task_id}/claim`
- `POST /boards/{board_id}/tasks/{task_id}/release`
- `POST /boards/{board_id}/tasks/{task_id}/move/{target_column_id}`

### Events & comments
- `GET /boards/{board_id}/tasks/{task_id}/events`
- `POST /boards/{board_id}/tasks/{task_id}/comment` — `{ "message": "..." }`

### API keys (admin)
- `GET /keys`
- `POST /keys`
- `DELETE /keys/{id}` (revokes)

## Development

```bash
cd backend
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

## License

MIT
