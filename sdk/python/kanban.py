#!/usr/bin/env python3
"""
kanban — Python SDK for HNR Kanban Board

Zero-dependency client library for the Kanban API.
Works with Python 3.8+ using only the standard library.

Quick start:
    from kanban import Kanban

    kb = Kanban("http://localhost:3002")

    # Create a board
    board = kb.create_board("Sprint 42")
    print(f"Board ID: {board['id']}")
    print(f"Manage key: {board['manage_key']}")

    # Connect to existing board
    kb = Kanban("http://localhost:3002", board_id="...", manage_key="kb_...")

    # Create tasks
    task = kb.create_task("Fix login bug", priority="high", labels=["bug"])
    kb.move_task(task["id"], column_id="<in-progress-column-id>")

    # Search
    results = kb.search("login")

Full docs: GET /api/v1/llms.txt or /.well-known/skills/kanban/SKILL.md
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.parse
import urllib.request
from typing import (
    Any,
    Dict,
    Generator,
    List,
    Optional,
    Union,
)


__version__ = "1.0.0"


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------


class KanbanError(Exception):
    """Base exception for Kanban API errors."""

    def __init__(self, message: str, status_code: int = 0, body: Any = None):
        super().__init__(message)
        self.status_code = status_code
        self.body = body


class NotFoundError(KanbanError):
    """Resource not found (404)."""
    pass


class AuthError(KanbanError):
    """Manage key required or invalid (401/403)."""
    pass


class ConflictError(KanbanError):
    """Conflict — e.g. WIP limit exceeded, circular dependency (409)."""
    pass


class ValidationError(KanbanError):
    """Invalid input (422)."""
    pass


class RateLimitError(KanbanError):
    """Rate limited (429). Check retry_after."""

    def __init__(self, message: str, retry_after: float = 0, **kwargs):
        super().__init__(message, **kwargs)
        self.retry_after = retry_after


# ---------------------------------------------------------------------------
# Client
# ---------------------------------------------------------------------------


class Kanban:
    """
    Kanban board client.

    Args:
        base_url: Base URL of the Kanban service (e.g. "http://localhost:3002").
        board_id: Default board ID for operations.
        manage_key: Default manage key for write operations.
        timeout: Request timeout in seconds (default 30).

    Environment variables (fallbacks):
        KANBAN_URL: Base URL
        KANBAN_BOARD_ID: Default board ID
        KANBAN_MANAGE_KEY: Default manage key
    """

    def __init__(
        self,
        base_url: Optional[str] = None,
        board_id: Optional[str] = None,
        manage_key: Optional[str] = None,
        timeout: int = 30,
    ):
        self.base_url = (base_url or os.environ.get("KANBAN_URL", "")).rstrip("/")
        if not self.base_url:
            raise ValueError("base_url required (or set KANBAN_URL)")
        self.board_id = board_id or os.environ.get("KANBAN_BOARD_ID")
        self.manage_key = manage_key or os.environ.get("KANBAN_MANAGE_KEY")
        self.timeout = timeout

    # ------------------------------------------------------------------
    # HTTP helpers
    # ------------------------------------------------------------------

    def _url(self, path: str) -> str:
        return f"{self.base_url}/api/v1{path}"

    def _headers(self, auth: bool = False, key: Optional[str] = None) -> Dict[str, str]:
        h: Dict[str, str] = {"Content-Type": "application/json"}
        k = key or (self.manage_key if auth else None)
        if k:
            h["Authorization"] = f"Bearer {k}"
        return h

    def _request(
        self,
        method: str,
        path: str,
        body: Any = None,
        auth: bool = False,
        key: Optional[str] = None,
        params: Optional[Dict[str, Any]] = None,
    ) -> Any:
        url = self._url(path)
        if params:
            filtered = {k: v for k, v in params.items() if v is not None}
            if filtered:
                url += "?" + urllib.parse.urlencode(filtered)

        data = json.dumps(body).encode() if body is not None else None
        headers = self._headers(auth=auth, key=key)
        if data is None:
            headers.pop("Content-Type", None)

        req = urllib.request.Request(url, data=data, headers=headers, method=method)

        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                raw = resp.read()
                if not raw:
                    return None
                return json.loads(raw)
        except urllib.error.HTTPError as e:
            body_text = e.read().decode("utf-8", errors="replace") if e.fp else ""
            try:
                err_body = json.loads(body_text)
            except (json.JSONDecodeError, ValueError):
                err_body = {"error": body_text}

            msg = err_body.get("error", body_text) if isinstance(err_body, dict) else body_text

            if e.code == 401 or e.code == 403:
                raise AuthError(msg, status_code=e.code, body=err_body)
            if e.code == 404:
                raise NotFoundError(msg, status_code=e.code, body=err_body)
            if e.code == 409:
                raise ConflictError(msg, status_code=e.code, body=err_body)
            if e.code == 422:
                raise ValidationError(msg, status_code=e.code, body=err_body)
            if e.code == 429:
                retry = float(e.headers.get("Retry-After", 0))
                raise RateLimitError(msg, retry_after=retry, status_code=e.code, body=err_body)
            raise KanbanError(msg, status_code=e.code, body=err_body)

    def _get(self, path: str, params: Optional[Dict[str, Any]] = None, **kw) -> Any:
        return self._request("GET", path, params=params, **kw)

    def _post(self, path: str, body: Any = None, **kw) -> Any:
        return self._request("POST", path, body=body, **kw)

    def _put(self, path: str, body: Any = None, **kw) -> Any:
        return self._request("PUT", path, body=body, **kw)

    def _patch(self, path: str, body: Any = None, **kw) -> Any:
        return self._request("PATCH", path, body=body, **kw)

    def _delete(self, path: str, **kw) -> Any:
        return self._request("DELETE", path, **kw)

    # ------------------------------------------------------------------
    # Board resolution helper
    # ------------------------------------------------------------------

    def _board(self, board_id: Optional[str] = None) -> str:
        bid = board_id or self.board_id
        if not bid:
            raise ValueError("board_id required (pass it or set on constructor)")
        return bid

    # ==================================================================
    # Health
    # ==================================================================

    def health(self) -> dict:
        """GET /api/v1/health — service health check."""
        return self._get("/health")

    # ==================================================================
    # Boards
    # ==================================================================

    def create_board(
        self,
        name: str,
        description: str = "",
        columns: Optional[List[str]] = None,
        is_public: bool = False,
        require_display_name: bool = False,
    ) -> dict:
        """
        Create a new board. Returns id, manage_key, view_url, manage_url.

        The manage_key is only shown once — save it!
        """
        body: Dict[str, Any] = {"name": name, "description": description}
        if columns:
            body["columns"] = columns
        if is_public:
            body["is_public"] = True
        if require_display_name:
            body["require_display_name"] = True
        return self._post("/boards", body)

    def list_boards(self) -> List[dict]:
        """List all public boards."""
        return self._get("/boards")

    def get_board(self, board_id: Optional[str] = None, key: Optional[str] = None) -> dict:
        """
        Get board details including columns and task counts.
        Pass manage_key to see full board settings.
        """
        bid = self._board(board_id)
        params = {"key": key} if key else None
        return self._get(f"/boards/{bid}", params=params)

    def update_board(
        self,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        name: Optional[str] = None,
        description: Optional[str] = None,
        is_public: Optional[bool] = None,
        require_display_name: Optional[bool] = None,
        quick_done_column_id: Optional[str] = None,
        quick_done_auto_archive: Optional[bool] = None,
        quick_reassign_column_id: Optional[str] = None,
        quick_reassign_to: Optional[str] = None,
    ) -> dict:
        """Update board settings (manage key required)."""
        bid = self._board(board_id)
        body: Dict[str, Any] = {}
        if name is not None:
            body["name"] = name
        if description is not None:
            body["description"] = description
        if is_public is not None:
            body["is_public"] = is_public
        if require_display_name is not None:
            body["require_display_name"] = require_display_name
        if quick_done_column_id is not None:
            body["quick_done_column_id"] = quick_done_column_id
        if quick_done_auto_archive is not None:
            body["quick_done_auto_archive"] = quick_done_auto_archive
        if quick_reassign_column_id is not None:
            body["quick_reassign_column_id"] = quick_reassign_column_id
        if quick_reassign_to is not None:
            body["quick_reassign_to"] = quick_reassign_to
        return self._patch(f"/boards/{bid}", body, auth=True, key=key)

    def archive_board(self, board_id: Optional[str] = None, *, key: Optional[str] = None) -> dict:
        """Archive a board (manage key required)."""
        bid = self._board(board_id)
        return self._post(f"/boards/{bid}/archive", auth=True, key=key)

    def unarchive_board(self, board_id: Optional[str] = None, *, key: Optional[str] = None) -> dict:
        """Unarchive a board (manage key required)."""
        bid = self._board(board_id)
        return self._post(f"/boards/{bid}/unarchive", auth=True, key=key)

    # ==================================================================
    # Columns
    # ==================================================================

    def create_column(
        self,
        name: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        position: Optional[int] = None,
        wip_limit: Optional[int] = None,
    ) -> dict:
        """Create a new column on the board (manage key required)."""
        bid = self._board(board_id)
        body: Dict[str, Any] = {"name": name}
        if position is not None:
            body["position"] = position
        if wip_limit is not None:
            body["wip_limit"] = wip_limit
        return self._post(f"/boards/{bid}/columns", body, auth=True, key=key)

    def update_column(
        self,
        column_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        name: Optional[str] = None,
        wip_limit: Optional[int] = ...,  # sentinel: None = clear, ... = don't send
    ) -> dict:
        """Update a column (manage key required). Set wip_limit=None to clear."""
        bid = self._board(board_id)
        body: Dict[str, Any] = {}
        if name is not None:
            body["name"] = name
        if wip_limit is not ...:
            body["wip_limit"] = wip_limit
        return self._patch(f"/boards/{bid}/columns/{column_id}", body, auth=True, key=key)

    def delete_column(
        self,
        column_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
    ) -> None:
        """Delete a column (manage key required). Fails if column has tasks."""
        bid = self._board(board_id)
        self._delete(f"/boards/{bid}/columns/{column_id}", auth=True, key=key)

    def reorder_columns(
        self,
        column_ids: List[str],
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
    ) -> dict:
        """Reorder columns by providing ordered list of IDs (manage key required)."""
        bid = self._board(board_id)
        return self._post(
            f"/boards/{bid}/columns/reorder",
            {"column_ids": column_ids},
            auth=True,
            key=key,
        )

    # ==================================================================
    # Tasks
    # ==================================================================

    def create_task(
        self,
        title: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        description: str = "",
        column_id: Optional[str] = None,
        priority: Union[int, str] = 0,
        position: Optional[int] = None,
        assigned_to: Optional[str] = None,
        labels: Optional[List[str]] = None,
        metadata: Optional[dict] = None,
        due_at: Optional[str] = None,
        actor_name: str = "",
    ) -> dict:
        """
        Create a task on the board (manage key required).

        Priority: 0=low, 1=medium, 2=high, 3=critical (or string names).
        """
        bid = self._board(board_id)
        body: Dict[str, Any] = {
            "title": title,
            "description": description,
            "priority": priority,
        }
        if column_id:
            body["column_id"] = column_id
        if position is not None:
            body["position"] = position
        if assigned_to:
            body["assigned_to"] = assigned_to
        if labels:
            body["labels"] = labels
        if metadata:
            body["metadata"] = metadata
        if due_at:
            body["due_at"] = due_at
        if actor_name:
            body["actor_name"] = actor_name
        return self._post(f"/boards/{bid}/tasks", body, auth=True, key=key)

    def list_tasks(
        self,
        board_id: Optional[str] = None,
        *,
        column_id: Optional[str] = None,
        priority: Optional[int] = None,
        assigned_to: Optional[str] = None,
        label: Optional[str] = None,
        include_archived: bool = False,
        updated_before: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[dict]:
        """List tasks on a board with optional filters."""
        bid = self._board(board_id)
        params: Dict[str, Any] = {}
        if column_id:
            params["column"] = column_id
        if priority is not None:
            params["priority"] = priority
        if assigned_to:
            params["assigned"] = assigned_to
        if label:
            params["label"] = label
        if include_archived:
            params["archived"] = "true"
        if updated_before:
            params["updated_before"] = updated_before
        if limit is not None:
            params["limit"] = limit
        if offset is not None:
            params["offset"] = offset
        return self._get(f"/boards/{bid}/tasks", params=params)

    def get_task(self, task_id: str, board_id: Optional[str] = None) -> dict:
        """Get a single task by ID."""
        bid = self._board(board_id)
        return self._get(f"/boards/{bid}/tasks/{task_id}")

    def update_task(
        self,
        task_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        title: Optional[str] = None,
        description: Optional[str] = None,
        column_id: Optional[str] = None,
        priority: Optional[Union[int, str]] = None,
        assigned_to: Optional[str] = None,
        labels: Optional[List[str]] = None,
        metadata: Optional[dict] = None,
        due_at: Optional[str] = None,
        actor_name: Optional[str] = None,
    ) -> dict:
        """Update a task (manage key required). Only provided fields are changed."""
        bid = self._board(board_id)
        body: Dict[str, Any] = {}
        if title is not None:
            body["title"] = title
        if description is not None:
            body["description"] = description
        if column_id is not None:
            body["column_id"] = column_id
        if priority is not None:
            body["priority"] = priority
        if assigned_to is not None:
            body["assigned_to"] = assigned_to
        if labels is not None:
            body["labels"] = labels
        if metadata is not None:
            body["metadata"] = metadata
        if due_at is not None:
            body["due_at"] = due_at
        if actor_name is not None:
            body["actor_name"] = actor_name
        return self._patch(f"/boards/{bid}/tasks/{task_id}", body, auth=True, key=key)

    def delete_task(
        self,
        task_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        actor: Optional[str] = None,
    ) -> None:
        """Delete a task (manage key required)."""
        bid = self._board(board_id)
        params = {"actor": actor} if actor else None
        self._delete(f"/boards/{bid}/tasks/{task_id}", auth=True, key=key, params=params)

    def archive_task(
        self,
        task_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        actor: Optional[str] = None,
    ) -> dict:
        """Archive a task (manage key required)."""
        bid = self._board(board_id)
        params = {"actor": actor} if actor else None
        return self._post(f"/boards/{bid}/tasks/{task_id}/archive", auth=True, key=key, params=params)

    def unarchive_task(
        self,
        task_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        actor: Optional[str] = None,
    ) -> dict:
        """Unarchive a task (manage key required)."""
        bid = self._board(board_id)
        params = {"actor": actor} if actor else None
        return self._post(f"/boards/{bid}/tasks/{task_id}/unarchive", auth=True, key=key, params=params)

    # ==================================================================
    # Task actions (claim, release, move, reorder)
    # ==================================================================

    def claim_task(
        self,
        task_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        actor: Optional[str] = None,
    ) -> dict:
        """Claim a task — mark it as being worked on by an agent/person."""
        bid = self._board(board_id)
        params = {"actor": actor} if actor else None
        return self._post(f"/boards/{bid}/tasks/{task_id}/claim", auth=True, key=key, params=params)

    def release_task(
        self,
        task_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        actor: Optional[str] = None,
    ) -> dict:
        """Release a claimed task."""
        bid = self._board(board_id)
        params = {"actor": actor} if actor else None
        return self._post(f"/boards/{bid}/tasks/{task_id}/release", auth=True, key=key, params=params)

    def move_task(
        self,
        task_id: str,
        column_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        actor: Optional[str] = None,
    ) -> dict:
        """Move a task to a different column (manage key required)."""
        bid = self._board(board_id)
        params = {"actor": actor} if actor else None
        return self._post(
            f"/boards/{bid}/tasks/{task_id}/move/{column_id}",
            auth=True,
            key=key,
            params=params,
        )

    def reorder_task(
        self,
        task_id: str,
        position: int,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        column_id: Optional[str] = None,
    ) -> dict:
        """Reorder a task within its column (or move + reorder)."""
        bid = self._board(board_id)
        body: Dict[str, Any] = {"position": position}
        if column_id:
            body["column_id"] = column_id
        return self._post(f"/boards/{bid}/tasks/{task_id}/reorder", body, auth=True, key=key)

    # ==================================================================
    # Batch operations
    # ==================================================================

    def batch(
        self,
        operations: List[dict],
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        actor_name: Optional[str] = None,
    ) -> dict:
        """
        Execute batch operations on tasks (manage key required).

        Each operation is a dict with "action" and operation-specific fields:
        - {"action": "move", "task_ids": [...], "column_id": "..."}
        - {"action": "update", "task_ids": [...], "priority": 2, ...}
        - {"action": "delete", "task_ids": [...]}

        Max 50 operations per request.
        """
        bid = self._board(board_id)
        body: Dict[str, Any] = {"operations": operations}
        if actor_name:
            body["actor_name"] = actor_name
        return self._post(f"/boards/{bid}/tasks/batch", body, auth=True, key=key)

    # Convenience batch methods

    def batch_move(
        self,
        task_ids: List[str],
        column_id: str,
        board_id: Optional[str] = None,
        **kw,
    ) -> dict:
        """Move multiple tasks to a column in one request."""
        return self.batch(
            [{"action": "move", "task_ids": task_ids, "column_id": column_id}],
            board_id,
            **kw,
        )

    def batch_update(
        self,
        task_ids: List[str],
        board_id: Optional[str] = None,
        *,
        priority: Optional[int] = None,
        assigned_to: Optional[str] = None,
        labels: Optional[List[str]] = None,
        due_at: Optional[str] = None,
        **kw,
    ) -> dict:
        """Update fields on multiple tasks in one request."""
        op: Dict[str, Any] = {"action": "update", "task_ids": task_ids}
        if priority is not None:
            op["priority"] = priority
        if assigned_to is not None:
            op["assigned_to"] = assigned_to
        if labels is not None:
            op["labels"] = labels
        if due_at is not None:
            op["due_at"] = due_at
        return self.batch([op], board_id, **kw)

    def batch_delete(
        self,
        task_ids: List[str],
        board_id: Optional[str] = None,
        **kw,
    ) -> dict:
        """Delete multiple tasks in one request."""
        return self.batch(
            [{"action": "delete", "task_ids": task_ids}],
            board_id,
            **kw,
        )

    # ==================================================================
    # Comments (via task events)
    # ==================================================================

    def comment(
        self,
        task_id: str,
        message: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        actor_name: str = "",
    ) -> dict:
        """Add a comment to a task (manage key required)."""
        bid = self._board(board_id)
        body: Dict[str, Any] = {"message": message}
        if actor_name:
            body["actor_name"] = actor_name
        return self._post(f"/boards/{bid}/tasks/{task_id}/comment", body, auth=True, key=key)

    def get_task_events(
        self,
        task_id: str,
        board_id: Optional[str] = None,
    ) -> List[dict]:
        """Get all events for a task (comments, moves, updates, etc.)."""
        bid = self._board(board_id)
        return self._get(f"/boards/{bid}/tasks/{task_id}/events")

    # ==================================================================
    # Activity feed
    # ==================================================================

    def get_activity(
        self,
        board_id: Optional[str] = None,
        *,
        after: Optional[int] = None,
        since: Optional[str] = None,
        limit: Optional[int] = None,
        mentioned: Optional[str] = None,
    ) -> List[dict]:
        """
        Get board activity feed (newest first).

        Use after=<seq> for cursor-based pagination.
        Use since=<ISO-8601> for time-based filtering.
        Use mentioned=<name> to filter by @mentions.
        """
        bid = self._board(board_id)
        params: Dict[str, Any] = {}
        if after is not None:
            params["after"] = after
        if since:
            params["since"] = since
        if limit is not None:
            params["limit"] = limit
        if mentioned:
            params["mentioned"] = mentioned
        return self._get(f"/boards/{bid}/activity", params=params)

    # ==================================================================
    # Search
    # ==================================================================

    def search(
        self,
        query: str,
        board_id: Optional[str] = None,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> dict:
        """
        Search tasks by title/description.

        Returns: {"query", "tasks", "total", "limit", "offset"}
        """
        bid = self._board(board_id)
        params: Dict[str, Any] = {"q": query}
        if limit is not None:
            params["limit"] = limit
        if offset is not None:
            params["offset"] = offset
        return self._get(f"/boards/{bid}/tasks/search", params=params)

    # ==================================================================
    # Task dependencies
    # ==================================================================

    def create_dependency(
        self,
        blocker_task_id: str,
        blocked_task_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        note: str = "",
        actor_name: str = "",
    ) -> dict:
        """
        Create a dependency: blocker must complete before blocked can proceed.
        Circular dependencies are rejected.
        """
        bid = self._board(board_id)
        body: Dict[str, Any] = {
            "blocker_task_id": blocker_task_id,
            "blocked_task_id": blocked_task_id,
        }
        if note:
            body["note"] = note
        if actor_name:
            body["actor_name"] = actor_name
        return self._post(f"/boards/{bid}/dependencies", body, auth=True, key=key)

    def list_dependencies(self, board_id: Optional[str] = None) -> List[dict]:
        """List all dependencies on a board."""
        bid = self._board(board_id)
        return self._get(f"/boards/{bid}/dependencies")

    def delete_dependency(
        self,
        dependency_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
    ) -> None:
        """Delete a dependency (manage key required)."""
        bid = self._board(board_id)
        self._delete(f"/boards/{bid}/dependencies/{dependency_id}", auth=True, key=key)

    # ==================================================================
    # Webhooks
    # ==================================================================

    def create_webhook(
        self,
        url: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        events: Optional[List[str]] = None,
    ) -> dict:
        """
        Create a webhook (manage key required).
        Returns the webhook secret (shown only once).
        """
        bid = self._board(board_id)
        body: Dict[str, Any] = {"url": url}
        if events:
            body["events"] = events
        return self._post(f"/boards/{bid}/webhooks", body, auth=True, key=key)

    def list_webhooks(
        self,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
    ) -> List[dict]:
        """List webhooks on a board (manage key required)."""
        bid = self._board(board_id)
        return self._get(f"/boards/{bid}/webhooks", auth=True, key=key)

    def update_webhook(
        self,
        webhook_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
        url: Optional[str] = None,
        events: Optional[List[str]] = None,
        active: Optional[bool] = None,
    ) -> dict:
        """Update a webhook (manage key required)."""
        bid = self._board(board_id)
        body: Dict[str, Any] = {}
        if url is not None:
            body["url"] = url
        if events is not None:
            body["events"] = events
        if active is not None:
            body["active"] = active
        return self._patch(f"/boards/{bid}/webhooks/{webhook_id}", body, auth=True, key=key)

    def delete_webhook(
        self,
        webhook_id: str,
        board_id: Optional[str] = None,
        *,
        key: Optional[str] = None,
    ) -> None:
        """Delete a webhook (manage key required)."""
        bid = self._board(board_id)
        self._delete(f"/boards/{bid}/webhooks/{webhook_id}", auth=True, key=key)

    # ==================================================================
    # SSE streaming
    # ==================================================================

    def stream(
        self,
        board_id: Optional[str] = None,
    ) -> Generator[dict, None, None]:
        """
        Stream real-time board events via SSE.

        Yields dicts with 'event' and 'data' keys.
        Events: task.created, task.updated, task.moved, task.deleted,
                task.archived, task.unarchived, task.claimed, task.released,
                column.created, column.updated, column.deleted, column.reordered,
                comment.added, board.updated, board.archived, board.unarchived
        """
        bid = self._board(board_id)
        url = f"{self.base_url}/api/v1/boards/{bid}/events/stream"
        req = urllib.request.Request(url, headers={"Accept": "text/event-stream"})

        with urllib.request.urlopen(req, timeout=None) as resp:
            event_type = ""
            data_lines: List[str] = []

            for raw_line in resp:
                line = raw_line.decode("utf-8", errors="replace").rstrip("\n\r")

                if line.startswith("event:"):
                    event_type = line[6:].strip()
                elif line.startswith("data:"):
                    data_lines.append(line[5:].strip())
                elif line == "":
                    # End of event
                    if data_lines:
                        raw_data = "\n".join(data_lines)
                        try:
                            parsed = json.loads(raw_data)
                        except json.JSONDecodeError:
                            parsed = raw_data
                        yield {"event": event_type or "message", "data": parsed}
                    event_type = ""
                    data_lines = []

    # ==================================================================
    # Discovery
    # ==================================================================

    def llms_txt(self) -> str:
        """GET /llms.txt — AI-readable API summary."""
        url = f"{self.base_url}/llms.txt"
        req = urllib.request.Request(url)
        with urllib.request.urlopen(req, timeout=self.timeout) as resp:
            return resp.read().decode()

    def openapi(self) -> dict:
        """GET /api/v1/openapi.json — OpenAPI 3.0 spec."""
        return self._get("/openapi.json")

    def skills(self) -> dict:
        """GET /.well-known/skills/index.json — Agent skills discovery."""
        url = f"{self.base_url}/.well-known/skills/index.json"
        req = urllib.request.Request(url)
        with urllib.request.urlopen(req, timeout=self.timeout) as resp:
            return json.loads(resp.read())

    # ==================================================================
    # Convenience helpers
    # ==================================================================

    def find_column(
        self,
        name: str,
        board_id: Optional[str] = None,
    ) -> Optional[dict]:
        """Find a column by name (case-insensitive). Returns column dict or None."""
        board = self.get_board(board_id)
        name_lower = name.lower()
        for col in board.get("columns", []):
            if col["name"].lower() == name_lower:
                return col
        return None

    def move_task_to(
        self,
        task_id: str,
        column_name: str,
        board_id: Optional[str] = None,
        **kw,
    ) -> dict:
        """Move a task to a column by name (convenience wrapper)."""
        col = self.find_column(column_name, board_id)
        if not col:
            raise NotFoundError(f"Column '{column_name}' not found")
        return self.move_task(task_id, col["id"], board_id, **kw)

    def quick_task(
        self,
        title: str,
        board_id: Optional[str] = None,
        *,
        column_name: Optional[str] = None,
        **kw,
    ) -> dict:
        """
        Create a task with column lookup by name.
        If column_name is provided, resolves to column_id automatically.
        """
        if column_name:
            col = self.find_column(column_name, board_id)
            if not col:
                raise NotFoundError(f"Column '{column_name}' not found")
            kw["column_id"] = col["id"]
        return self.create_task(title, board_id, **kw)


# ---------------------------------------------------------------------------
# Demo
# ---------------------------------------------------------------------------


def _demo():
    """Quick demo — run with: python kanban.py"""
    import sys

    url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:3002"
    kb = Kanban(url)

    print(f"Health: {kb.health()}")

    board = kb.create_board("SDK Demo Board", description="Created by Python SDK")
    print(f"\nCreated board: {board['name']} (id: {board['id']})")
    print(f"Manage key: {board['manage_key']}")
    print(f"View URL: {board['view_url']}")

    # Set manage key for subsequent calls
    kb.board_id = board["id"]
    kb.manage_key = board["manage_key"]

    # Create tasks
    t1 = kb.create_task("Design API", priority="high", labels=["backend"])
    t2 = kb.create_task("Write tests", priority="medium", labels=["testing"])
    t3 = kb.create_task("Deploy", priority="low", assigned_to="Nanook")
    print(f"\nCreated 3 tasks: {t1['title']}, {t2['title']}, {t3['title']}")

    # Move task by column name
    kb.move_task_to(t1["id"], "In Progress")
    print(f"Moved '{t1['title']}' to In Progress")

    # Add comment
    kb.comment(t1["id"], "Starting API design now", actor_name="Nanook")
    print("Added comment")

    # Search
    results = kb.search("API")
    print(f"\nSearch 'API': {results['total']} result(s)")

    # Activity feed
    activity = kb.get_activity(limit=5)
    print(f"Recent activity: {len(activity)} events")

    # Batch move remaining tasks
    kb.batch_move([t2["id"], t3["id"]], kb.find_column("Up Next")["id"])
    print("Batch moved 2 tasks to Up Next")

    # Cleanup
    kb.batch_delete([t1["id"], t2["id"], t3["id"]])
    kb.archive_board()
    print("\nCleaned up — archived board")


if __name__ == "__main__":
    _demo()
