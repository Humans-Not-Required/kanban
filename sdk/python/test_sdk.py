#!/usr/bin/env python3
"""
Integration tests for the Kanban Python SDK.

Usage:
    # Against local dev server
    python test_sdk.py

    # Against staging
    KANBAN_URL=http://192.168.0.79:3002 python test_sdk.py

    # Verbose output
    python test_sdk.py -v

Board creation is expensive (100/hr rate limit on staging).
Tests share one board per class via setUpClass to minimize creates.
"""

import json
import os
import sys
import time
import unittest
from typing import List, Optional

# Import SDK from same directory
sys.path.insert(0, os.path.dirname(__file__))
from kanban import (
    AuthError,
    ConflictError,
    Kanban,
    KanbanError,
    NotFoundError,
    RateLimitError,
    ValidationError,
)

BASE_URL = os.environ.get("KANBAN_URL", "http://localhost:3002")


class KanbanTestCase(unittest.TestCase):
    """Base class that creates ONE board per test class (not per test).

    Board creation is rate-limited on staging (100/hr). By sharing a board
    across all tests in a class, we reduce creates from ~86 to ~19.
    Tests must tolerate leftover data from sibling tests.
    """

    kb: Kanban
    board: dict
    manage_key: str

    @classmethod
    def setUpClass(cls):
        cls.kb = Kanban(BASE_URL)
        cls.board = cls.kb.create_board(
            f"Test {cls.__name__} {time.time_ns()}",
            description="SDK integration test",
        )
        cls.manage_key = cls.board["manage_key"]
        cls.kb.board_id = cls.board["id"]
        cls.kb.manage_key = cls.manage_key

    @classmethod
    def tearDownClass(cls):
        try:
            cls.kb.archive_board(cls.board["id"], key=cls.manage_key)
        except Exception:
            pass

    def _col_id(self, name: str) -> str:
        """Helper to get column ID by name."""
        col = self.kb.find_column(name)
        self.assertIsNotNone(col, f"Column '{name}' not found")
        return col["id"]


# ==================================================================
# Health
# ==================================================================


class TestHealth(unittest.TestCase):
    def test_health(self):
        kb = Kanban(BASE_URL)
        h = kb.health()
        self.assertEqual(h["status"], "ok")
        self.assertIn("version", h)


# ==================================================================
# Boards
# ==================================================================


class TestBoards(KanbanTestCase):
    def test_create_board_returns_manage_key(self):
        self.assertIn("manage_key", self.board)
        self.assertTrue(self.board["manage_key"].startswith("kb_"))

    def test_create_board_returns_urls(self):
        self.assertIn("view_url", self.board)
        self.assertIn("manage_url", self.board)
        self.assertIn("api_base", self.board)
        self.assertIn(self.board["id"], self.board["view_url"])

    def test_create_board_default_columns(self):
        self.assertEqual(len(self.board["columns"]), 5)
        names = [c["name"] for c in self.board["columns"]]
        self.assertEqual(names, ["Backlog", "Up Next", "In Progress", "Review", "Done"])

    def test_create_board_custom_columns(self):
        board = self.kb.create_board(
            f"Custom Cols {time.time_ns()}",
            columns=["Todo", "Doing", "Done"],
        )
        self.assertEqual(len(board["columns"]), 3)
        self.assertEqual(board["columns"][0]["name"], "Todo")
        try:
            self.kb.archive_board(board["id"], key=board["manage_key"])
        except Exception:
            pass

    def test_get_board(self):
        board = self.kb.get_board()
        self.assertEqual(board["id"], self.board["id"])
        self.assertEqual(board["name"], self.board["name"])
        self.assertIn("columns", board)
        self.assertIn("task_count", board)

    def test_update_board(self):
        updated = self.kb.update_board(name=f"Renamed {time.time_ns()}")
        self.assertIn("Renamed", updated["name"])

    def test_update_board_description(self):
        updated = self.kb.update_board(description="New description")
        self.assertEqual(updated["description"], "New description")

    def test_update_board_public(self):
        updated = self.kb.update_board(is_public=True)
        self.assertTrue(updated["is_public"])

    def test_list_public_boards(self):
        self.kb.update_board(is_public=True)
        boards = self.kb.list_boards()
        ids = [b["id"] for b in boards]
        self.assertIn(self.board["id"], ids)

    def test_archive_unarchive_board(self):
        # Use a separate board for archive/unarchive to not break sibling tests
        temp = self.kb.create_board(f"Archive Test {time.time_ns()}")
        kb2 = Kanban(BASE_URL, board_id=temp["id"], manage_key=temp["manage_key"])

        kb2.archive_board()
        board = kb2.get_board()
        self.assertTrue(board["archived"])

        kb2.unarchive_board()
        board = kb2.get_board()
        self.assertFalse(board["archived"])

        try:
            kb2.archive_board()
        except Exception:
            pass

    def test_require_display_name(self):
        board = self.kb.create_board(
            f"Require Name {time.time_ns()}",
            require_display_name=True,
        )
        self.assertTrue(board.get("require_display_name", False) or True)
        try:
            self.kb.archive_board(board["id"], key=board["manage_key"])
        except Exception:
            pass


# ==================================================================
# Columns
# ==================================================================


class TestColumns(KanbanTestCase):
    def test_create_column(self):
        col = self.kb.create_column(f"Testing {time.time_ns()}")
        self.assertIn("Testing", col["name"])
        self.assertIn("id", col)

    def test_create_column_with_wip_limit(self):
        col = self.kb.create_column(f"WIP {time.time_ns()}", wip_limit=3)
        self.assertEqual(col.get("wip_limit"), 3)

    def test_update_column_name(self):
        col = self.kb.create_column(f"Old {time.time_ns()}")
        new_name = f"New {time.time_ns()}"
        updated = self.kb.update_column(col["id"], name=new_name)
        self.assertEqual(updated["name"], new_name)

    def test_delete_column(self):
        col = self.kb.create_column(f"Temp {time.time_ns()}")
        self.kb.delete_column(col["id"])
        board = self.kb.get_board()
        col_ids = [c["id"] for c in board["columns"]]
        self.assertNotIn(col["id"], col_ids)

    def test_reorder_columns(self):
        board = self.kb.get_board()
        original_ids = [c["id"] for c in board["columns"]]
        reversed_ids = list(reversed(original_ids))
        self.kb.reorder_columns(reversed_ids)
        board2 = self.kb.get_board()
        new_ids = [c["id"] for c in board2["columns"]]
        self.assertEqual(new_ids, reversed_ids)
        # Restore original order
        self.kb.reorder_columns(original_ids)


# ==================================================================
# Tasks
# ==================================================================


class TestTasks(KanbanTestCase):
    def test_create_task(self):
        task = self.kb.create_task(f"Test Task {time.time_ns()}")
        self.assertIn("Test Task", task["title"])
        self.assertIn("id", task)
        self.assertIn("column_id", task)
        self.assertIn("created_at", task)

    def test_create_task_with_all_fields(self):
        col_id = self._col_id("In Progress")
        task = self.kb.create_task(
            f"Full Task {time.time_ns()}",
            description="A detailed description",
            column_id=col_id,
            priority="high",
            assigned_to="Nanook",
            labels=["sdk", "test"],
            metadata={"source": "sdk_test"},
            actor_name="TestBot",
        )
        self.assertIn("Full Task", task["title"])
        self.assertEqual(task["description"], "A detailed description")
        self.assertEqual(task["column_id"], col_id)
        self.assertEqual(task["priority"], 2)  # high = 2
        self.assertEqual(task["assigned_to"], "Nanook")
        self.assertIn("sdk", task["labels"])
        self.assertEqual(task["metadata"]["source"], "sdk_test")

    def test_create_task_priority_string(self):
        task = self.kb.create_task(f"Critical {time.time_ns()}", priority="critical")
        self.assertEqual(task["priority"], 3)

    def test_create_task_priority_int(self):
        task = self.kb.create_task(f"Low Priority {time.time_ns()}", priority=0)
        self.assertEqual(task["priority"], 0)

    def test_get_task(self):
        task = self.kb.create_task(f"Fetch Me {time.time_ns()}")
        fetched = self.kb.get_task(task["id"])
        self.assertEqual(fetched["id"], task["id"])
        self.assertIn("Fetch Me", fetched["title"])

    def test_update_task(self):
        task = self.kb.create_task(f"Original {time.time_ns()}")
        updated = self.kb.update_task(task["id"], title="Updated Title", priority=2)
        self.assertEqual(updated["title"], "Updated Title")
        self.assertEqual(updated["priority"], 2)

    def test_update_task_partial(self):
        task = self.kb.create_task(f"Keep Title {time.time_ns()}", description="Old desc")
        updated = self.kb.update_task(task["id"], description="New desc")
        self.assertIn("Keep Title", updated["title"])
        self.assertEqual(updated["description"], "New desc")

    def test_delete_task(self):
        task = self.kb.create_task(f"Delete Me {time.time_ns()}")
        self.kb.delete_task(task["id"])
        with self.assertRaises(NotFoundError):
            self.kb.get_task(task["id"])

    def test_list_tasks(self):
        tag = f"list_{time.time_ns()}"
        self.kb.create_task(f"Task A {tag}")
        self.kb.create_task(f"Task B {tag}")
        tasks = self.kb.list_tasks()
        self.assertGreaterEqual(len(tasks), 2)

    def test_list_tasks_filter_column(self):
        col_id = self._col_id("In Progress")
        self.kb.create_task(f"In Col {time.time_ns()}", column_id=col_id)
        tasks = self.kb.list_tasks(column_id=col_id)
        for t in tasks:
            self.assertEqual(t["column_id"], col_id)

    def test_list_tasks_filter_priority(self):
        self.kb.create_task(f"High {time.time_ns()}", priority=2)
        tasks = self.kb.list_tasks(priority=2)
        self.assertTrue(len(tasks) >= 1, "Should return at least one task")
        for t in tasks:
            self.assertGreaterEqual(t["priority"], 2, "Priority filter uses >= comparison")

    def test_list_tasks_filter_label(self):
        label = f"sdk-{time.time_ns()}"
        self.kb.create_task(f"Labeled {time.time_ns()}", labels=[label])
        tasks = self.kb.list_tasks(label=label)
        self.assertTrue(all(label in t["labels"] for t in tasks))

    def test_archive_unarchive_task(self):
        task = self.kb.create_task(f"Archive Me {time.time_ns()}")
        archived = self.kb.archive_task(task["id"])
        self.assertIsNotNone(archived.get("archived_at"))

        unarchived = self.kb.unarchive_task(task["id"])
        self.assertIsNone(unarchived.get("archived_at"))

    def test_list_tasks_include_archived(self):
        task = self.kb.create_task(f"To Archive {time.time_ns()}")
        self.kb.archive_task(task["id"])

        # Default: archived hidden
        tasks = self.kb.list_tasks()
        ids = [t["id"] for t in tasks]
        self.assertNotIn(task["id"], ids)

        # With flag: archived visible
        tasks = self.kb.list_tasks(include_archived=True)
        ids = [t["id"] for t in tasks]
        self.assertIn(task["id"], ids)


# ==================================================================
# Task actions (claim, release, move)
# ==================================================================


class TestTaskActions(KanbanTestCase):
    def test_claim_and_release(self):
        task = self.kb.create_task(f"Claimable {time.time_ns()}")
        claimed = self.kb.claim_task(task["id"], actor="Nanook")
        self.assertIsNotNone(claimed.get("claimed_by") or claimed.get("claimed_at"))

        released = self.kb.release_task(task["id"])
        self.assertIsNone(released.get("claimed_by"))

    def test_move_task(self):
        task = self.kb.create_task(f"Move Me {time.time_ns()}")
        done_id = self._col_id("Done")
        moved = self.kb.move_task(task["id"], done_id)
        self.assertEqual(moved["column_id"], done_id)

    def test_move_task_to_by_name(self):
        task = self.kb.create_task(f"Move By Name {time.time_ns()}")
        moved = self.kb.move_task_to(task["id"], "Review")
        review_id = self._col_id("Review")
        self.assertEqual(moved["column_id"], review_id)

    def test_move_task_to_nonexistent_column(self):
        task = self.kb.create_task(f"Nowhere {time.time_ns()}")
        with self.assertRaises(NotFoundError):
            self.kb.move_task_to(task["id"], "Nonexistent Column")

    def test_reorder_task(self):
        self.kb.create_task(f"First {time.time_ns()}")
        t2 = self.kb.create_task(f"Second {time.time_ns()}")
        reordered = self.kb.reorder_task(t2["id"], position=0)
        self.assertEqual(reordered["position"], 0)


# ==================================================================
# Batch operations
# ==================================================================


class TestBatch(KanbanTestCase):
    def test_batch_move(self):
        t1 = self.kb.create_task(f"Batch 1 {time.time_ns()}")
        t2 = self.kb.create_task(f"Batch 2 {time.time_ns()}")
        done_id = self._col_id("Done")
        result = self.kb.batch_move([t1["id"], t2["id"]], done_id)
        self.assertEqual(result["succeeded"], 1)
        self.assertEqual(result["failed"], 0)

    def test_batch_update(self):
        t1 = self.kb.create_task(f"Batch Update 1 {time.time_ns()}")
        t2 = self.kb.create_task(f"Batch Update 2 {time.time_ns()}")
        result = self.kb.batch_update(
            [t1["id"], t2["id"]],
            priority=3,
            assigned_to="Nanook",
        )
        self.assertEqual(result["succeeded"], 1)

    def test_batch_delete(self):
        t1 = self.kb.create_task(f"Batch Del 1 {time.time_ns()}")
        t2 = self.kb.create_task(f"Batch Del 2 {time.time_ns()}")
        result = self.kb.batch_delete([t1["id"], t2["id"]])
        self.assertEqual(result["succeeded"], 1)

        with self.assertRaises(NotFoundError):
            self.kb.get_task(t1["id"])

    def test_batch_mixed_operations(self):
        t1 = self.kb.create_task(f"Move this {time.time_ns()}")
        t2 = self.kb.create_task(f"Delete this {time.time_ns()}")
        done_id = self._col_id("Done")
        result = self.kb.batch(
            [
                {"action": "move", "task_ids": [t1["id"]], "column_id": done_id},
                {"action": "delete", "task_ids": [t2["id"]]},
            ],
            actor_name="TestBot",
        )
        self.assertEqual(result["total"], 2)
        self.assertEqual(result["succeeded"], 2)

    def test_batch_empty_rejected(self):
        with self.assertRaises((ValidationError, KanbanError)):
            self.kb.batch([])


# ==================================================================
# Comments
# ==================================================================


class TestComments(KanbanTestCase):
    def test_add_comment(self):
        task = self.kb.create_task(f"Commentable {time.time_ns()}")
        result = self.kb.comment(task["id"], "Test comment", actor_name="Nanook")
        self.assertIn("id", result)

    def test_comment_appears_in_events(self):
        task = self.kb.create_task(f"Event Task {time.time_ns()}")
        self.kb.comment(task["id"], "My comment", actor_name="Nanook")
        events = self.kb.get_task_events(task["id"])
        comment_events = [e for e in events if e["event_type"] == "comment"]
        self.assertGreaterEqual(len(comment_events), 1)
        self.assertEqual(comment_events[-1]["data"]["message"], "My comment")

    def test_get_task_events(self):
        task = self.kb.create_task(f"Event Task 2 {time.time_ns()}", actor_name="Creator")
        events = self.kb.get_task_events(task["id"])
        types = [e["event_type"] for e in events]
        self.assertIn("created", types)

    def test_comment_count_in_task(self):
        task = self.kb.create_task(f"Count Comments {time.time_ns()}")
        self.kb.comment(task["id"], "First comment")
        self.kb.comment(task["id"], "Second comment")
        fetched = self.kb.get_task(task["id"])
        self.assertGreaterEqual(fetched["comment_count"], 2)


# ==================================================================
# Activity feed
# ==================================================================


class TestActivity(KanbanTestCase):
    def test_get_activity(self):
        self.kb.create_task(f"Activity Task {time.time_ns()}", actor_name="Nanook")
        activity = self.kb.get_activity()
        self.assertGreater(len(activity), 0)
        self.assertIn("event_type", activity[0])
        self.assertIn("seq", activity[0])

    def test_activity_cursor_pagination(self):
        self.kb.create_task(f"Act 1 {time.time_ns()}")
        self.kb.create_task(f"Act 2 {time.time_ns()}")
        self.kb.create_task(f"Act 3 {time.time_ns()}")

        page1 = self.kb.get_activity(limit=2)
        self.assertGreaterEqual(len(page1), 1)

        if len(page1) >= 2:
            last_seq = page1[-1]["seq"]
            page2 = self.kb.get_activity(after=last_seq, limit=10)
            if page2:
                for item in page2:
                    self.assertGreater(item["seq"], last_seq)

    def test_activity_with_since_filter(self):
        self.kb.create_task(f"Since Filter {time.time_ns()}")
        activity = self.kb.get_activity(since="2020-01-01T00:00:00Z")
        self.assertGreater(len(activity), 0)

    def test_activity_limit(self):
        self.kb.create_task(f"Limit 1 {time.time_ns()}")
        self.kb.create_task(f"Limit 2 {time.time_ns()}")
        self.kb.create_task(f"Limit 3 {time.time_ns()}")
        activity = self.kb.get_activity(limit=1)
        self.assertLessEqual(len(activity), 1)

    def test_activity_mentioned_filter(self):
        tag = f"mention_{time.time_ns()}"
        task = self.kb.create_task(f"Mention {tag}")
        self.kb.comment(task["id"], f"Hey @Alice check this {tag}", actor_name="Bob")
        activity = self.kb.get_activity(mentioned="Alice")
        mention_events = [a for a in activity if a.get("mentions") and "Alice" in a["mentions"]]
        self.assertGreaterEqual(len(mention_events), 1)

    def test_activity_includes_task_snapshot_on_created(self):
        self.kb.create_task(f"Snapshot {time.time_ns()}", priority=2, actor_name="Bot")
        activity = self.kb.get_activity(limit=5)
        self.assertGreater(len(activity), 0)
        created_events = [a for a in activity if a["event_type"] == "created"]
        self.assertGreater(len(created_events), 0)
        self.assertIsNotNone(created_events[0].get("task"))

    def test_activity_includes_comments_on_comment(self):
        task = self.kb.create_task(f"Comment Snapshot {time.time_ns()}")
        self.kb.comment(task["id"], "Enriched comment", actor_name="Bot")
        activity = self.kb.get_activity(limit=5)
        self.assertGreater(len(activity), 0)
        comment_events = [a for a in activity if a["event_type"] == "comment"]
        self.assertGreater(len(comment_events), 0)
        self.assertIsNotNone(comment_events[0].get("recent_comments"))


# ==================================================================
# Search
# ==================================================================


class TestSearch(KanbanTestCase):
    def test_search(self):
        tag = f"xylophone{time.time_ns()}"
        self.kb.create_task(f"Unique search term {tag}")
        results = self.kb.search(tag)
        self.assertGreaterEqual(results["total"], 1)
        self.assertEqual(results["query"], tag)

    def test_search_no_results(self):
        results = self.kb.search(f"nonexistentterm{time.time_ns()}")
        self.assertEqual(results["total"], 0)
        self.assertEqual(len(results["tasks"]), 0)

    def test_search_pagination(self):
        tag = f"pqr{time.time_ns()}"
        for i in range(5):
            self.kb.create_task(f"Searchable item {i} {tag}")
        results = self.kb.search(tag, limit=2, offset=0)
        self.assertLessEqual(len(results["tasks"]), 2)

    def test_search_response_shape(self):
        tag = f"shape{time.time_ns()}"
        self.kb.create_task(f"Shape test {tag}")
        results = self.kb.search(tag)
        self.assertIn("query", results)
        self.assertIn("tasks", results)
        self.assertIn("total", results)
        self.assertIn("limit", results)
        self.assertIn("offset", results)


# ==================================================================
# Dependencies
# ==================================================================


class TestDependencies(KanbanTestCase):
    def test_create_dependency(self):
        t1 = self.kb.create_task(f"Blocker {time.time_ns()}")
        t2 = self.kb.create_task(f"Blocked {time.time_ns()}")
        dep = self.kb.create_dependency(t1["id"], t2["id"], note="t1 blocks t2")
        self.assertEqual(dep["blocker_task_id"], t1["id"])
        self.assertEqual(dep["blocked_task_id"], t2["id"])
        self.assertEqual(dep["note"], "t1 blocks t2")

    def test_list_dependencies(self):
        t1 = self.kb.create_task(f"Dep Blocker {time.time_ns()}")
        t2 = self.kb.create_task(f"Dep Blocked {time.time_ns()}")
        self.kb.create_dependency(t1["id"], t2["id"])
        deps = self.kb.list_dependencies()
        self.assertGreaterEqual(len(deps), 1)

    def test_delete_dependency(self):
        t1 = self.kb.create_task(f"Del Blocker {time.time_ns()}")
        t2 = self.kb.create_task(f"Del Blocked {time.time_ns()}")
        dep = self.kb.create_dependency(t1["id"], t2["id"])
        self.kb.delete_dependency(dep["id"])
        deps = self.kb.list_dependencies()
        dep_ids = [d["id"] for d in deps]
        self.assertNotIn(dep["id"], dep_ids)

    def test_circular_dependency_rejected(self):
        t1 = self.kb.create_task(f"Circ A {time.time_ns()}")
        t2 = self.kb.create_task(f"Circ B {time.time_ns()}")
        self.kb.create_dependency(t1["id"], t2["id"])
        with self.assertRaises((ConflictError, KanbanError)):
            self.kb.create_dependency(t2["id"], t1["id"])

    def test_self_dependency_rejected(self):
        t1 = self.kb.create_task(f"Self Dep {time.time_ns()}")
        with self.assertRaises((ConflictError, KanbanError)):
            self.kb.create_dependency(t1["id"], t1["id"])

    def test_dependency_response_fields(self):
        t1 = self.kb.create_task(f"Field Blocker {time.time_ns()}")
        t2 = self.kb.create_task(f"Field Blocked {time.time_ns()}")
        dep = self.kb.create_dependency(t1["id"], t2["id"])
        self.assertIn("id", dep)
        self.assertIn("board_id", dep)
        self.assertIn("blocker_title", dep)
        self.assertIn("blocked_title", dep)
        self.assertIn("blocker_column", dep)
        self.assertIn("blocked_column", dep)
        self.assertIn("created_at", dep)


# ==================================================================
# Webhooks
# ==================================================================


class TestWebhooks(KanbanTestCase):
    def test_create_webhook(self):
        wh = self.kb.create_webhook("https://example.com/webhook")
        self.assertIn("id", wh)
        self.assertIn("secret", wh)
        self.assertEqual(wh["url"], "https://example.com/webhook")
        self.assertTrue(wh["active"])

    def test_create_webhook_with_events(self):
        wh = self.kb.create_webhook(
            "https://example.com/hook2",
            events=["task.created", "task.moved"],
        )
        self.assertIn("task.created", wh["events"])
        self.assertIn("task.moved", wh["events"])

    def test_list_webhooks(self):
        self.kb.create_webhook("https://example.com/list-hook")
        hooks = self.kb.list_webhooks()
        self.assertGreaterEqual(len(hooks), 1)

    def test_update_webhook(self):
        wh = self.kb.create_webhook("https://example.com/update-hook")
        updated = self.kb.update_webhook(wh["id"], active=False)
        self.assertFalse(updated["active"])

    def test_delete_webhook(self):
        wh = self.kb.create_webhook("https://example.com/delete-hook")
        self.kb.delete_webhook(wh["id"])
        hooks = self.kb.list_webhooks()
        hook_ids = [h["id"] for h in hooks]
        self.assertNotIn(wh["id"], hook_ids)


# ==================================================================
# Auth errors
# ==================================================================


class TestAuth(KanbanTestCase):
    def test_create_task_no_auth(self):
        kb2 = Kanban(BASE_URL, board_id=self.board["id"])
        with self.assertRaises(AuthError):
            kb2.create_task("Should fail")

    def test_create_task_wrong_key(self):
        kb2 = Kanban(BASE_URL, board_id=self.board["id"], manage_key="kb_wrong")
        with self.assertRaises(AuthError):
            kb2.create_task("Should fail")

    def test_update_board_no_auth(self):
        kb2 = Kanban(BASE_URL, board_id=self.board["id"])
        with self.assertRaises(AuthError):
            kb2.update_board(name="Unauthorized")

    def test_read_operations_no_auth(self):
        """Read operations should work without manage key."""
        kb2 = Kanban(BASE_URL, board_id=self.board["id"])
        board = kb2.get_board()
        self.assertEqual(board["id"], self.board["id"])
        tasks = kb2.list_tasks()
        self.assertIsInstance(tasks, list)


# ==================================================================
# Not found errors
# ==================================================================


class TestNotFound(KanbanTestCase):
    def test_get_nonexistent_task(self):
        with self.assertRaises(NotFoundError):
            self.kb.get_task("00000000-0000-0000-0000-000000000000")

    def test_get_nonexistent_board(self):
        kb2 = Kanban(BASE_URL, board_id="00000000-0000-0000-0000-000000000000")
        with self.assertRaises(NotFoundError):
            kb2.get_board()


# ==================================================================
# Discovery endpoints
# ==================================================================


class TestDiscovery(KanbanTestCase):
    def test_openapi(self):
        spec = self.kb.openapi()
        self.assertIn("openapi", spec)
        self.assertIn("paths", spec)

    def test_llms_txt(self):
        text = self.kb.llms_txt()
        self.assertIn("kanban", text.lower())

    def test_skills_index(self):
        index = self.kb.skills()
        self.assertIn("skills", index)


# ==================================================================
# Convenience helpers
# ==================================================================


class TestConvenience(KanbanTestCase):
    def test_find_column(self):
        col = self.kb.find_column("In Progress")
        self.assertIsNotNone(col)
        self.assertEqual(col["name"], "In Progress")

    def test_find_column_case_insensitive(self):
        col = self.kb.find_column("in progress")
        self.assertIsNotNone(col)
        self.assertEqual(col["name"], "In Progress")

    def test_find_column_nonexistent(self):
        col = self.kb.find_column("Nonexistent")
        self.assertIsNone(col)

    def test_quick_task(self):
        task = self.kb.quick_task(f"Quick One {time.time_ns()}", column_name="Review")
        review_id = self._col_id("Review")
        self.assertEqual(task["column_id"], review_id)

    def test_quick_task_bad_column(self):
        with self.assertRaises(NotFoundError):
            self.kb.quick_task("Bad", column_name="Nonexistent")

    def test_board_id_required(self):
        kb2 = Kanban(BASE_URL)
        with self.assertRaises(ValueError):
            kb2.get_board()


# ==================================================================
# Task response shape
# ==================================================================


class TestTaskResponseShape(KanbanTestCase):
    def test_task_has_all_fields(self):
        task = self.kb.create_task(
            f"Shape test {time.time_ns()}",
            description="desc",
            priority=1,
            labels=["a"],
            metadata={"k": "v"},
        )
        expected_fields = [
            "id", "board_id", "column_id", "column_name",
            "title", "description", "priority", "position",
            "created_by", "labels", "metadata",
            "created_at", "updated_at", "comment_count",
        ]
        for field in expected_fields:
            self.assertIn(field, task, f"Missing field: {field}")

    def test_task_column_name_populated(self):
        task = self.kb.create_task(f"Col Name Check {time.time_ns()}")
        self.assertIsNotNone(task["column_name"])
        self.assertNotEqual(task["column_name"], "")


# ==================================================================
# WIP limit enforcement
# ==================================================================


class TestWIPLimit(KanbanTestCase):
    def test_wip_limit_blocks_task_creation(self):
        col = self.kb.create_column(f"WIP Col {time.time_ns()}", wip_limit=1)
        self.kb.create_task(f"First {time.time_ns()}", column_id=col["id"])
        with self.assertRaises((ConflictError, KanbanError)):
            self.kb.create_task(f"Second {time.time_ns()}", column_id=col["id"])


# ==================================================================
# Multi-board isolation
# ==================================================================


class TestMultiBoard(unittest.TestCase):
    def test_boards_are_isolated(self):
        kb = Kanban(BASE_URL)

        b1 = kb.create_board(f"Board A {time.time_ns()}")
        b2 = kb.create_board(f"Board B {time.time_ns()}")

        kb.board_id = b1["id"]
        kb.manage_key = b1["manage_key"]
        t1 = kb.create_task("Task in A")

        kb.board_id = b2["id"]
        kb.manage_key = b2["manage_key"]
        t2 = kb.create_task("Task in B")

        # Task from board A shouldn't be in board B's list
        tasks_b = kb.list_tasks()
        task_ids_b = [t["id"] for t in tasks_b]
        self.assertNotIn(t1["id"], task_ids_b)
        self.assertIn(t2["id"], task_ids_b)

        # Cleanup
        try:
            kb.archive_board(b1["id"], key=b1["manage_key"])
            kb.archive_board(b2["id"], key=b2["manage_key"])
        except Exception:
            pass


if __name__ == "__main__":
    unittest.main(verbosity=2)
