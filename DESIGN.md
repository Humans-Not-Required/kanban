# Kanban — Design Document

> See also: [Shared Design Principles](../docs/design-principles.md)

## Overview

A kanban board for task management, designed for both AI agents and humans. Zero-signup board creation with link-based access control.

## Auth Model: Resource-Scoped Tokens (No Accounts)

Follows the **Pastebin/Excalidraw model** — creating a board returns access tokens. No user accounts, no signup, no login.

### Access Rules

| Operation | Auth Required | Rationale |
|-----------|--------------|-----------|
| View board (read-only) | ❌ No | Anyone with the board URL can view |
| List public boards | ❌ No | Discovery/showcase |
| Create board | ❌ No | Returns a manage token |
| Edit board / manage tasks | 🔑 Board token | Scoped to that board |
| Delete board | 🔑 Board token | Scoped to that board |
| API access | 🔑 Board token | Same token, in Bearer header |

### How It Works

1. **Create a board** → API returns:
   ```json
   {
     "board_id": "uuid",
     "manage_key": "kb_uuid",
     "view_url": "/board/{board_id}",
     "manage_url": "/board/{board_id}?key={manage_key}",
     "api_base": "/api/boards/{board_id}"
   }
   ```

2. **View URL** (`/board/{uuid}`) — read-only access. Anyone with this link can see the board, its columns, and tasks. Cannot modify anything.

3. **Manage URL** (`/board/{uuid}?key={token}`) — full access. Can add/edit/move/delete tasks, manage columns, configure the board. The frontend detects the key in the URL and enables edit mode.

4. **API access** — use the manage token as `Authorization: Bearer {manage_key}` for programmatic access to all write endpoints on that board.

### Token Storage

- Management tokens are hashed and stored per-board in SQLite
- One board = one management token (v1 simplicity)
- Future: multiple tokens per board with different permissions (read-only share, collaborator, admin)

## User Flows

### AI Agent Flow
1. `POST /boards` with `{ name: "Sprint 42", columns: ["Todo", "In Progress", "Done"] }`
2. Gets back `board_id`, `manage_key`, `view_url`, `manage_url`
3. Uses `manage_key` as Bearer token for all subsequent API calls
4. Shares `view_url` with humans for read-only viewing
5. Shares `manage_url` with humans who need edit access

### Human Flow
1. Open the web UI → click "New Board"
2. Board created instantly — shown the board with a notification:
   > "Bookmark this URL to manage your board: [manage_url]. Anyone with the view link can see it."
3. Start adding columns and tasks immediately
4. Share the URL (view or manage) with others

### AI → Human Handoff
Agent creates a board, adds tasks, then sends the human a message:
> "Here's your sprint board: [manage_url]"

Human clicks the link → full board with tasks, ready to use. Zero friction.

## Board Visibility

- **Default: unlisted** — board exists but isn't discoverable. You need the UUID to find it.
- **Optional: public** — board appears in a public showcase/listing. Good for demo/community boards.
- Security through obscurity (UUID is unguessable) is fine for v1. Not meant for sensitive data.

## API Changes Needed (from current state)

Current implementation requires `AuthenticatedKey` (global API key) on every route. Changes needed:

1. **Replace global API key auth with per-board token auth**
2. **Board creation requires no auth** — returns the board's manage token
3. **Read endpoints require no auth** — just the board UUID in the URL
4. **Write endpoints require the board's manage token** — in Bearer header or `key` query param
5. **Remove global API key management** (create key, list keys, revoke keys)
6. **Add `key` query param support** — so manage URLs work in browsers
7. **Frontend: detect key in URL** → enable/disable edit mode accordingly

## Rate Limiting

IP-based rate limiting for board creation (prevent spam board creation). Generous limits — e.g., 10 boards/hour per IP. No rate limiting on viewing.

## Data Model Notes

- Each board has: `id` (UUID), `name`, `manage_key_hash`, `is_public`, `created_at`
- Columns belong to a board, ordered by position
- Tasks belong to a column, have title/description/priority/labels/assigned
- No user table in v1
