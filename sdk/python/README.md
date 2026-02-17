# Kanban Python SDK

Zero-dependency Python client for the HNR Kanban Board API.

## Quick Start

```python
from kanban import Kanban

kb = Kanban("http://localhost:3002")

# Create a board
board = kb.create_board("Sprint 42")
print(f"Manage key: {board['manage_key']}")  # Save this!

# Set defaults for convenience
kb.board_id = board["id"]
kb.manage_key = board["manage_key"]

# Create tasks
task = kb.create_task("Fix login bug", priority="high", labels=["bug"])
kb.move_task_to(task["id"], "In Progress")

# Search
results = kb.search("login")
print(f"Found {results['total']} tasks")
```

## Installation

Copy `kanban.py` into your project. No dependencies needed — uses only Python 3.8+ standard library.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `KANBAN_URL` | Base URL (e.g. `http://localhost:3002`) |
| `KANBAN_BOARD_ID` | Default board ID |
| `KANBAN_MANAGE_KEY` | Default manage key |

## API Coverage

### Boards
- `create_board()` — Create a new board (returns manage key)
- `list_boards()` — List public boards
- `get_board()` — Get board with columns and stats
- `update_board()` — Update settings (name, description, visibility, quick actions)
- `archive_board()` / `unarchive_board()`

### Columns
- `create_column()` — Add a column (with optional WIP limit)
- `update_column()` — Rename or change WIP limit
- `delete_column()` — Remove empty column
- `reorder_columns()` — Set column order

### Tasks
- `create_task()` — Create with title, priority, labels, metadata, due date
- `list_tasks()` — List with filters (column, priority, label, assigned, archived)
- `get_task()` — Get single task
- `update_task()` — Partial update
- `delete_task()` — Delete task
- `archive_task()` / `unarchive_task()`

### Task Actions
- `claim_task()` / `release_task()` — Claim/release task ownership
- `move_task()` — Move to column by ID
- `move_task_to()` — Move to column by name (convenience)
- `reorder_task()` — Change position within column

### Batch Operations
- `batch()` — Execute multiple operations atomically
- `batch_move()` — Move multiple tasks
- `batch_update()` — Update fields on multiple tasks
- `batch_delete()` — Delete multiple tasks

### Comments & Events
- `comment()` — Add a comment (with @mention extraction)
- `get_task_events()` — Get task history

### Activity Feed
- `get_activity()` — Board-wide activity (cursor pagination, @mention filter)

### Search
- `search()` — Full-text search with pagination

### Dependencies
- `create_dependency()` — Define blocker/blocked relationships
- `list_dependencies()` — List all dependencies
- `delete_dependency()` — Remove dependency

### Webhooks
- `create_webhook()` — Subscribe to events
- `list_webhooks()` / `update_webhook()` / `delete_webhook()`

### SSE Streaming
- `stream()` — Real-time event stream (generator)

### Discovery
- `llms_txt()` — AI-readable API docs
- `openapi()` — OpenAPI 3.0 spec
- `skills()` — Agent skills index

## Error Handling

```python
from kanban import AuthError, NotFoundError, ConflictError, RateLimitError

try:
    kb.create_task("Test")
except AuthError:
    print("Need manage key")
except ConflictError as e:
    print(f"WIP limit: {e}")
except RateLimitError as e:
    print(f"Retry in {e.retry_after}s")
```

## Running Tests

```bash
# Start local server
cd backend && BOARD_RATE_LIMIT=10000 cargo run

# Run tests
cd sdk/python && python test_sdk.py -v

# Against staging (rate limited to 10 boards/hr)
KANBAN_URL=http://192.168.0.79:3002 python test_sdk.py
```

## License

MIT
