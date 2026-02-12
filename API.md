# Kanban API Reference

Base URL: `/api/v1`

## Authentication

No accounts, no signup. Boards are the only resource and each has its own management token.

- **Create a board** → returns a `manage_key` (shown once — save it)
- **Read operations** (GET) → public, just need the board UUID
- **Write operations** (POST/PATCH/DELETE) → require `manage_key`

### Passing the Token

The manage key can be sent three ways (checked in order):

| Method | Example |
|--------|---------|
| `Authorization` header | `Authorization: Bearer kb_abc123` |
| `X-API-Key` header | `X-API-Key: kb_abc123` |
| `?key=` query param | `GET /api/v1/boards/{id}?key=kb_abc123` |

---

## System

### Health Check

```
GET /health
```

No auth. Returns service status.

```json
{ "status": "ok", "version": "0.1.0" }
```

### OpenAPI Spec

```
GET /openapi.json
```

### LLM-Friendly Docs

```
GET /llms.txt
```

---

## Boards

### Create Board

```
POST /boards
```

No auth required. Returns `manage_key` — **save it, it's shown only once**.

**Request:**

```json
{
  "name": "Sprint 1",
  "description": "Optional description",
  "columns": ["Todo", "Doing", "Done"],
  "is_public": false,
  "require_display_name": false
}
```

All fields except `name` are optional. If `columns` is omitted, defaults to: Backlog, Up Next, In Progress, Review, Done.

**Response** `201`:

```json
{
  "id": "uuid",
  "name": "Sprint 1",
  "description": "",
  "columns": [
    { "id": "uuid", "name": "Todo", "position": 0, "wip_limit": null, "task_count": 0 }
  ],
  "manage_key": "kb_abc123",
  "view_url": "/board/{id}",
  "manage_url": "/board/{id}?key=kb_abc123",
  "api_base": "/api/v1/boards/{id}",
  "created_at": "2026-02-12T00:00:00Z"
}
```

**Errors:** `RATE_LIMIT_EXCEEDED` (429), `EMPTY_NAME` (400)

**Rate limit:** 10 boards per hour per IP (configurable via `BOARD_RATE_LIMIT` env var).

### List Public Boards

```
GET /boards
GET /boards?include_archived=true
```

No auth. Returns boards where `is_public = true`.

**Response** `200`: Array of `BoardSummary`:

```json
[
  {
    "id": "uuid",
    "name": "Sprint 1",
    "description": "",
    "task_count": 42,
    "archived": false,
    "is_public": true,
    "created_at": "2026-02-12T00:00:00Z"
  }
]
```

### Get Board

```
GET /boards/{id}
```

No auth. Returns full board details including columns.

**Response** `200`:

```json
{
  "id": "uuid",
  "name": "Sprint 1",
  "description": "",
  "columns": [
    { "id": "uuid", "name": "Todo", "position": 0, "wip_limit": null, "task_count": 5 }
  ],
  "task_count": 42,
  "archived": false,
  "is_public": false,
  "require_display_name": false,
  "quick_done_column_id": null,
  "quick_done_auto_archive": false,
  "quick_reassign_column_id": null,
  "quick_reassign_to": null,
  "created_at": "2026-02-12T00:00:00Z",
  "updated_at": "2026-02-12T00:00:00Z"
}
```

**Errors:** `BOARD_NOT_FOUND` (404)

### Update Board

```
PATCH /boards/{id}
```

🔑 Auth required. All fields optional.

**Request:**

```json
{
  "name": "New Name",
  "description": "Updated description",
  "is_public": true,
  "require_display_name": true,
  "quick_done_column_id": "column-uuid",
  "quick_done_auto_archive": true,
  "quick_reassign_column_id": "column-uuid",
  "quick_reassign_to": "agent-name"
}
```

**Response** `200`: Full `BoardResponse`.

**Errors:** `INVALID_INPUT` (400), `INVALID_COLUMN` (400, if quick_done/reassign column doesn't exist)

### Archive / Unarchive Board

```
POST /boards/{id}/archive
POST /boards/{id}/unarchive
```

🔑 Auth required.

**Response** `200`: `{ "message": "Board archived" }` or `{ "message": "Board unarchived" }`

**Errors:** `ALREADY_ARCHIVED` (400), `NOT_ARCHIVED` (400)

---

## Columns

### Create Column

```
POST /boards/{id}/columns
```

🔑 Auth required.

**Request:**

```json
{
  "name": "In Review",
  "position": 2,
  "wip_limit": 5
}
```

`position` and `wip_limit` are optional. If position is omitted, appends to the end.

**Response** `201`: `ColumnResponse`

### Update Column

```
PATCH /boards/{id}/columns/{colId}
```

🔑 Auth required. All fields optional.

**Request:**

```json
{
  "name": "Renamed Column",
  "wip_limit": 10
}
```

Set `wip_limit` to `null` to remove the limit.

**Response** `200`: `ColumnResponse`

**Errors:** `COLUMN_NOT_FOUND` (404)

### Delete Column

```
DELETE /boards/{id}/columns/{colId}
```

🔑 Auth required. Column must be empty (no tasks). Cannot delete the last column.

**Response** `200`: `{ "message": "Column deleted" }`

**Errors:** `COLUMN_NOT_FOUND` (404), `COLUMN_NOT_EMPTY` (400), `LAST_COLUMN` (400)

### Reorder Columns

```
POST /boards/{id}/columns/reorder
```

🔑 Auth required. Send the full ordered list of column IDs.

**Request:**

```json
{
  "column_ids": ["col-uuid-1", "col-uuid-3", "col-uuid-2"]
}
```

First ID gets position 0, second gets position 1, etc.

**Response** `200`: Array of `ColumnResponse`

**Errors:** `INVALID_COLUMN_LIST` (400), `COLUMN_NOT_FOUND` (400)

---

## Tasks

### Create Task

```
POST /boards/{id}/tasks
```

🔑 Auth required. At least one of `title` or `description` must be non-empty.

**Request:**

```json
{
  "title": "Implement auth",
  "description": "Add JWT-based authentication",
  "column_id": "col-uuid",
  "priority": 2,
  "position": 0,
  "assigned_to": "Jordan",
  "labels": ["backend", "security"],
  "metadata": { "source": "github-issue-42" },
  "due_at": "2026-03-01T00:00:00Z",
  "actor_name": "Nanook"
}
```

All fields are optional except that at least `title` or `description` must be provided. If `column_id` is omitted, the task goes to the first column.

**Priority values:** 0 = low (default), 1 = medium, 2 = high, 3 = critical. Also accepts strings: `"low"`, `"medium"`, `"high"`, `"critical"`.

Labels are normalized to lowercase with dashes (e.g., "My Label" → "my-label").

**Response** `201`: `TaskResponse`

**Errors:** `EMPTY_TASK` (400), `INVALID_COLUMN` (400), `DISPLAY_NAME_REQUIRED` (400), `WIP_LIMIT_EXCEEDED` (409)

### List Tasks

```
GET /boards/{id}/tasks
```

No auth. Returns tasks for a board.

**Query parameters** (all optional):

| Param | Description |
|-------|-------------|
| `column` | Filter by column ID |
| `assigned` | Filter by assigned_to |
| `claimed` | Filter by claimed_by |
| `priority` | Filter by priority (integer) |
| `label` | Filter by label (exact match) |
| `archived` | `true` to include archived tasks (excluded by default) |
| `updated_before` | ISO-8601 timestamp — only tasks with `updated_at` before this time |
| `limit` | Max results (default 200, max 1000) |
| `offset` | Pagination offset |

**Response** `200`: Array of `TaskResponse`

### Search Tasks

```
GET /boards/{id}/tasks/search?q=auth
```

No auth. Full-text search across titles, descriptions, and labels.

**Query parameters:**

| Param | Description |
|-------|-------------|
| `q` | Search query (required) |
| `limit` | Max results (1–100, default 50) |
| `offset` | Pagination offset |

Additional filters (`column`, `assigned`, `claimed`, `priority`, `label`) can be combined with search.

**Response** `200`:

```json
{
  "query": "auth",
  "tasks": [...],
  "total": 3,
  "limit": 50,
  "offset": 0
}
```

**Errors:** `EMPTY_QUERY` (400)

### Get Task

```
GET /boards/{id}/tasks/{taskId}
```

No auth.

**Response** `200`: `TaskResponse`

### Update Task

```
PATCH /boards/{id}/tasks/{taskId}
```

🔑 Auth required. All fields optional. At least one of `title` or `description` must remain non-empty after update.

**Request:**

```json
{
  "title": "Updated title",
  "description": "Updated description",
  "column_id": "new-col-uuid",
  "priority": 3,
  "assigned_to": "Jordan",
  "labels": ["frontend", "urgent"],
  "metadata": { "sprint": 2 },
  "due_at": "2026-03-15T00:00:00Z",
  "actor_name": "Nanook"
}
```

**Response** `200`: `TaskResponse`

**Errors:** `EMPTY_TASK` (400), `DISPLAY_NAME_REQUIRED` (400)

### Delete Task

```
DELETE /boards/{id}/tasks/{taskId}?actor=Nanook
```

🔑 Auth required. The `actor` query param is optional (defaults to "anonymous").

**Response** `200`: `{ "message": "Task deleted" }`

**Errors:** `DISPLAY_NAME_REQUIRED` (400)

---

## Task Actions

### Claim Task

```
POST /boards/{id}/tasks/{taskId}/claim?actor=Nanook
```

🔑 Auth required. Marks the task as "actively being worked on" by the actor. Prevents other agents from claiming it.

**Response** `200`: `TaskResponse`

**Errors:** `ALREADY_CLAIMED` (409), `DISPLAY_NAME_REQUIRED` (400)

> **Claim vs Assign:** `assigned_to` = responsibility ("this is your task"). `claimed_by` = active lock ("I'm working on this right now"). Claims prevent conflicts in multi-agent coordination.

### Release Claim

```
POST /boards/{id}/tasks/{taskId}/release?actor=Nanook
```

🔑 Auth required. Releases the active claim on a task.

**Response** `200`: `TaskResponse`

**Errors:** `DISPLAY_NAME_REQUIRED` (400)

### Move Task

```
POST /boards/{id}/tasks/{taskId}/move/{columnId}?actor=Nanook
```

🔑 Auth required. Moves a task to a different column. The `actor` query param is optional.

**Response** `200`: `TaskResponse`

**Errors:** `INVALID_COLUMN` (400), `WIP_LIMIT_EXCEEDED` (409), `DISPLAY_NAME_REQUIRED` (400)

### Reorder Task

```
POST /boards/{id}/tasks/{taskId}/reorder?actor=Nanook
```

🔑 Auth required. Changes task position within a column, or moves to a different column at a specific position.

**Request:**

```json
{
  "position": 0,
  "column_id": "optional-new-col-uuid"
}
```

**Response** `200`: `TaskResponse`

**Errors:** `INVALID_COLUMN` (400), `WIP_LIMIT_EXCEEDED` (409), `DISPLAY_NAME_REQUIRED` (400)

### Archive / Unarchive Task

```
POST /boards/{id}/tasks/{taskId}/archive?actor=Nanook
POST /boards/{id}/tasks/{taskId}/unarchive?actor=Nanook
```

🔑 Auth required. Archives or restores a task. Archived tasks are excluded from list by default (use `?archived=true` to include them).

**Response** `200`: `TaskResponse`

**Errors:** `DISPLAY_NAME_REQUIRED` (400)

---

## Batch Operations

```
POST /boards/{id}/tasks/batch
```

🔑 Auth required. Perform multiple operations in one request (max 50 operations).

**Request:**

```json
{
  "actor_name": "Nanook",
  "operations": [
    {
      "action": "move",
      "task_ids": ["task-1", "task-2"],
      "column_id": "done-col-uuid"
    },
    {
      "action": "update",
      "task_ids": ["task-3"],
      "priority": 3,
      "assigned_to": "Jordan",
      "labels": ["urgent"],
      "due_at": "2026-03-01T00:00:00Z"
    },
    {
      "action": "delete",
      "task_ids": ["task-4", "task-5"]
    }
  ]
}
```

**Actions:** `move`, `update`, `delete`

**Response** `200`:

```json
{
  "total": 3,
  "succeeded": 3,
  "failed": 0,
  "results": [
    { "action": "move", "task_ids": ["task-1", "task-2"], "success": true, "affected": 2 },
    { "action": "update", "task_ids": ["task-3"], "success": true, "affected": 1 },
    { "action": "delete", "task_ids": ["task-4", "task-5"], "success": true, "affected": 2 }
  ]
}
```

**Errors:** `EMPTY_BATCH` (400), `DISPLAY_NAME_REQUIRED` (400)

---

## Comments & Events

### Post Comment

```
POST /boards/{id}/tasks/{taskId}/comment
```

🔑 Auth required.

**Request:**

```json
{
  "message": "This looks good. @Jordan can you review?",
  "actor_name": "Nanook"
}
```

**@mentions:** Use `@Name` or `@"Quoted Name"` in comment text. Mentions are extracted and stored automatically.

**Response** `201`:

```json
{
  "id": "event-uuid",
  "event_type": "comment",
  "actor": "Nanook",
  "data": {
    "message": "This looks good. @Jordan can you review?",
    "actor": "Nanook",
    "mentions": ["Jordan"]
  },
  "created_at": "2026-02-12T00:30:00Z"
}
```

**Errors:** `EMPTY_MESSAGE` (400), `DISPLAY_NAME_REQUIRED` (400)

### Get Task Events

```
GET /boards/{id}/tasks/{taskId}/events
```

No auth. Returns the complete activity log for a specific task.

**Response** `200`: Array of `TaskEventResponse`

---

## Board Activity

```
GET /boards/{id}/activity
```

No auth. Returns board-wide activity across all tasks.

**Query parameters:**

| Param | Description |
|-------|-------------|
| `since` | ISO-8601 timestamp — only events after this time |
| `after` | Sequence number — cursor-based pagination (recommended) |
| `limit` | Max results |
| `mentioned` | Filter to events mentioning this name |

**Cursor-based polling (recommended):** Store the highest `seq` from the response and use `?after={seq}` on the next poll. More reliable than timestamp-based `?since=` for incremental consumption.

**Response** `200`: Array of `BoardActivityItem`:

```json
[
  {
    "id": "event-uuid",
    "task_id": "task-uuid",
    "task_title": "Implement auth",
    "event_type": "comment",
    "actor": "Nanook",
    "data": { "message": "Done!", "actor": "Nanook", "mentions": [] },
    "created_at": "2026-02-12T00:30:00Z",
    "seq": 42,
    "task": { "...full TaskResponse..." },
    "recent_comments": [
      { "id": "uuid", "actor": "Nanook", "message": "Done!", "created_at": "..." }
    ],
    "mentions": []
  }
]
```

**Enriched events:**
- `created` events include a full `task` snapshot
- `comment` events include `task` snapshot, `recent_comments` (last 10, newest first), and `mentions`
- Other event types (`moved`, `archived`, `updated`, `deleted`) are lean (no snapshots)

---

## Real-Time Events (SSE)

```
GET /boards/{id}/events/stream
```

No auth. Server-Sent Events stream for live board updates. Heartbeat every 15 seconds.

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

**Example:**

```bash
curl -N http://localhost:8000/api/v1/boards/$BOARD_ID/events/stream
```

Buffer holds 256 events. If a client falls behind, it receives a `warning` event.

---

## Webhooks

### Create Webhook

```
POST /boards/{id}/webhooks
```

🔑 Auth required.

**Request:**

```json
{
  "url": "https://example.com/webhook",
  "events": ["task.created", "task.moved"]
}
```

If `events` is empty, all event types are delivered.

**Response** `201`:

```json
{
  "id": "wh-uuid",
  "board_id": "board-uuid",
  "url": "https://example.com/webhook",
  "secret": "whsec_abc123",
  "events": ["task.created", "task.moved"],
  "active": true,
  "failure_count": 0,
  "last_triggered_at": null,
  "created_at": "2026-02-12T00:00:00Z"
}
```

The `secret` is returned **only on creation**. Use it to verify deliveries.

**Errors:** `EMPTY_URL` (400), `INVALID_EVENT_TYPE` (400)

### List Webhooks

```
GET /boards/{id}/webhooks
```

🔑 Auth required.

**Response** `200`: Array of `WebhookResponse` (without `secret`)

### Update Webhook

```
PATCH /boards/{id}/webhooks/{whId}
```

🔑 Auth required. All fields optional.

```json
{
  "url": "https://new-url.com/webhook",
  "events": ["task.created"],
  "active": true
}
```

**Errors:** `EMPTY_URL` (400), `INVALID_EVENT_TYPE` (400)

### Delete Webhook

```
DELETE /boards/{id}/webhooks/{whId}
```

🔑 Auth required.

**Response** `200`: `{ "message": "Webhook deleted" }`

### Webhook Delivery

Every delivery is an HTTP POST with:

**Headers:**
- `X-Kanban-Signature: sha256=<hex-digest>` (HMAC-SHA256 of body using webhook secret)
- `X-Kanban-Event: task.created`
- `X-Kanban-Board: <board-id>`

**Payload:**

```json
{
  "event": "task.created",
  "board_id": "board-uuid",
  "data": { "title": "Fix bug", "task_id": "task-uuid" },
  "timestamp": "2026-02-12T00:00:00Z"
}
```

**Reliability:**
- 10-second timeout per delivery
- Auto-disabled after 10 consecutive failures
- Re-enable via `PATCH` with `{"active": true}`
- Asynchronous delivery

---

## Dependencies

### Create Dependency

```
POST /boards/{id}/dependencies
```

🔑 Auth required.

**Request:**

```json
{
  "blocker_task_id": "task-uuid-1",
  "blocked_task_id": "task-uuid-2",
  "note": "Auth must be done before API routes"
}
```

**Response** `201`: `DependencyResponse`

### List Dependencies

```
GET /boards/{id}/dependencies
GET /boards/{id}/dependencies?task=task-uuid
```

No auth. Optionally filter by task ID (returns dependencies where the task is blocker or blocked).

**Response** `200`: Array of `DependencyResponse`:

```json
[
  {
    "id": "dep-uuid",
    "board_id": "board-uuid",
    "blocker_task_id": "task-uuid-1",
    "blocker_title": "Implement auth",
    "blocker_column": "In Progress",
    "blocker_completed": false,
    "blocked_task_id": "task-uuid-2",
    "blocked_title": "Add API routes",
    "blocked_column": "Backlog",
    "note": "Auth must be done before API routes",
    "created_by": "Nanook",
    "created_at": "2026-02-12T00:00:00Z"
  }
]
```

### Delete Dependency

```
DELETE /boards/{id}/dependencies/{depId}
```

🔑 Auth required.

**Response** `200`: `{ "message": "Dependency deleted" }`

---

## WIP Limits

Columns can have optional work-in-progress limits. When a column is at capacity:

- Creating or moving a task into it returns `409 Conflict`
- Error code: `WIP_LIMIT_EXCEEDED`
- Agents should handle this by moving tasks out first or choosing a different column
- Set `wip_limit` to `null` to remove the limit

---

## Display Name Enforcement

Boards with `require_display_name: true` reject write operations that don't include a non-empty, non-"anonymous" actor name.

- Error code: `DISPLAY_NAME_REQUIRED` (400)
- Applies to: task creation, updates, moves, deletes, claims, releases, archives, comments, reorders, and batch operations

---

## Error Format

All errors return JSON:

```json
{
  "error": "Human-readable message",
  "code": "MACHINE_READABLE_CODE",
  "status": 400
}
```

### Error Codes

| Code | Status | Description |
|------|--------|-------------|
| `EMPTY_NAME` | 400 | Board name is empty |
| `EMPTY_TASK` | 400 | Both title and description are empty |
| `EMPTY_MESSAGE` | 400 | Comment message is empty |
| `EMPTY_QUERY` | 400 | Search query is empty |
| `EMPTY_URL` | 400 | Webhook URL is empty |
| `EMPTY_BATCH` | 400 | No operations in batch request |
| `INVALID_INPUT` | 400 | General validation error |
| `INVALID_COLUMN` | 400 | Referenced column doesn't exist |
| `INVALID_COLUMN_LIST` | 400 | Reorder list doesn't match board columns |
| `INVALID_EVENT_TYPE` | 400 | Unknown webhook event type |
| `DISPLAY_NAME_REQUIRED` | 400 | Board requires a display name |
| `BOARD_NOT_FOUND` | 404 | Board doesn't exist |
| `COLUMN_NOT_FOUND` | 404 | Column doesn't exist |
| `TASK_NOT_FOUND` | 404 | Task doesn't exist |
| `ALREADY_CLAIMED` | 409 | Task is already claimed by someone |
| `ALREADY_ARCHIVED` | 400 | Board is already archived |
| `NOT_ARCHIVED` | 400 | Board is not archived |
| `WIP_LIMIT_EXCEEDED` | 409 | Column is at WIP capacity |
| `RATE_LIMIT_EXCEEDED` | 429 | Too many board creations from this IP |
| `UNAUTHORIZED` | 401 | Missing or invalid manage key |

---

## Object Reference

### TaskResponse

```json
{
  "id": "uuid",
  "board_id": "uuid",
  "column_id": "uuid",
  "column_name": "In Progress",
  "title": "Task title",
  "description": "Markdown description",
  "priority": 2,
  "position": 0,
  "created_by": "Nanook",
  "assigned_to": "Jordan",
  "claimed_by": null,
  "claimed_at": null,
  "labels": ["backend", "security"],
  "metadata": {},
  "due_at": null,
  "completed_at": null,
  "archived_at": null,
  "created_at": "2026-02-12T00:00:00Z",
  "updated_at": "2026-02-12T00:00:00Z",
  "comment_count": 3
}
```

### ColumnResponse

```json
{
  "id": "uuid",
  "name": "In Progress",
  "position": 2,
  "wip_limit": 5,
  "task_count": 3
}
```

### BoardActivityItem

```json
{
  "id": "event-uuid",
  "task_id": "task-uuid",
  "task_title": "Task title",
  "event_type": "created",
  "actor": "Nanook",
  "data": {},
  "created_at": "2026-02-12T00:00:00Z",
  "seq": 42,
  "task": null,
  "recent_comments": null,
  "mentions": null
}
```

Fields `task`, `recent_comments`, and `mentions` are only present on enriched events (see [Board Activity](#board-activity)).
