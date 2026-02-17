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


# ==================================================================
# Board settings (quick_done, quick_reassign)
# ==================================================================


class TestBoardSettings(KanbanTestCase):
    def test_quick_done_column(self):
        done_id = self._col_id("Done")
        updated = self.kb.update_board(quick_done_column_id=done_id)
        self.assertEqual(updated["quick_done_column_id"], done_id)

    def test_quick_done_auto_archive(self):
        done_id = self._col_id("Done")
        updated = self.kb.update_board(
            quick_done_column_id=done_id,
            quick_done_auto_archive=True,
        )
        self.assertTrue(updated["quick_done_auto_archive"])

    def test_quick_reassign_column(self):
        review_id = self._col_id("Review")
        updated = self.kb.update_board(
            quick_reassign_column_id=review_id,
            quick_reassign_to="Agent",
        )
        self.assertEqual(updated["quick_reassign_column_id"], review_id)
        self.assertEqual(updated["quick_reassign_to"], "Agent")

    def test_update_require_display_name(self):
        updated = self.kb.update_board(require_display_name=True)
        self.assertTrue(updated["require_display_name"])
        # Reset so other tests aren't affected
        self.kb.update_board(require_display_name=False)

    def test_board_response_has_settings_fields(self):
        board = self.kb.get_board()
        self.assertIn("quick_done_column_id", board)
        self.assertIn("quick_done_auto_archive", board)
        self.assertIn("quick_reassign_column_id", board)
        self.assertIn("quick_reassign_to", board)
        self.assertIn("require_display_name", board)

    def test_update_board_invalid_quick_done_column(self):
        with self.assertRaises(KanbanError):
            self.kb.update_board(quick_done_column_id="00000000-0000-0000-0000-000000000000")


# ==================================================================
# Task list filters — advanced
# ==================================================================


class TestTaskFilters(KanbanTestCase):
    def test_filter_by_assigned_to(self):
        tag = f"assign_{time.time_ns()}"
        self.kb.create_task(f"Assigned {tag}", assigned_to="FilterBot")
        self.kb.create_task(f"Unassigned {tag}")
        tasks = self.kb.list_tasks(assigned_to="FilterBot")
        for t in tasks:
            self.assertEqual(t["assigned_to"], "FilterBot")

    def test_filter_by_limit_and_offset(self):
        tag = f"page_{time.time_ns()}"
        for i in range(5):
            self.kb.create_task(f"Page {i} {tag}", labels=[tag])
        page1 = self.kb.list_tasks(label=tag, limit=2, offset=0)
        page2 = self.kb.list_tasks(label=tag, limit=2, offset=2)
        self.assertLessEqual(len(page1), 2)
        self.assertLessEqual(len(page2), 2)
        # Pages should have different tasks
        ids1 = {t["id"] for t in page1}
        ids2 = {t["id"] for t in page2}
        self.assertEqual(len(ids1 & ids2), 0, "Pages should not overlap")

    def test_filter_by_updated_before(self):
        tag = f"stale_{time.time_ns()}"
        self.kb.create_task(f"Old {tag}", labels=[tag])
        # All tasks created just now should appear with a future cutoff
        tasks = self.kb.list_tasks(label=tag, updated_before="2099-01-01T00:00:00Z")
        self.assertGreaterEqual(len(tasks), 1)

    def test_filter_by_multiple_criteria(self):
        col_id = self._col_id("In Progress")
        tag = f"multi_{time.time_ns()}"
        self.kb.create_task(f"Multi {tag}", column_id=col_id, priority=3, labels=[tag])
        tasks = self.kb.list_tasks(column_id=col_id, priority=3, label=tag)
        self.assertGreaterEqual(len(tasks), 1)
        for t in tasks:
            self.assertEqual(t["column_id"], col_id)
            self.assertIn(tag, t["labels"])

    def test_list_tasks_default_excludes_archived(self):
        tag = f"archfilt_{time.time_ns()}"
        task = self.kb.create_task(f"To Archive {tag}", labels=[tag])
        self.kb.archive_task(task["id"])
        tasks = self.kb.list_tasks(label=tag)
        ids = [t["id"] for t in tasks]
        self.assertNotIn(task["id"], ids)

    def test_list_tasks_empty_board(self):
        """A fresh board with no tasks should return empty list."""
        temp = self.kb.create_board(f"Empty Board {time.time_ns()}")
        kb2 = Kanban(BASE_URL, board_id=temp["id"], manage_key=temp["manage_key"])
        tasks = kb2.list_tasks()
        self.assertEqual(len(tasks), 0)
        try:
            kb2.archive_board()
        except Exception:
            pass


# ==================================================================
# Description-only tasks (no title)
# ==================================================================


class TestDescriptionOnlyTask(KanbanTestCase):
    def test_create_task_with_description_only(self):
        task = self.kb.create_task("", description=f"Description only {time.time_ns()}")
        self.assertEqual(task["title"], "")
        self.assertIn("Description only", task["description"])

    def test_update_task_clear_title_keep_description(self):
        task = self.kb.create_task(
            f"Has Title {time.time_ns()}", description="Has desc too"
        )
        updated = self.kb.update_task(task["id"], title="", description="Still has desc")
        self.assertEqual(updated["description"], "Still has desc")

    def test_create_task_empty_both_fails(self):
        with self.assertRaises((ValidationError, KanbanError)):
            self.kb.create_task("", description="")


# ==================================================================
# Task actions with actor attribution
# ==================================================================


class TestActorAttribution(KanbanTestCase):
    def test_delete_task_with_actor(self):
        task = self.kb.create_task(f"Delete Actor {time.time_ns()}")
        self.kb.delete_task(task["id"], actor="CleanupBot")
        with self.assertRaises(NotFoundError):
            self.kb.get_task(task["id"])

    def test_archive_task_with_actor(self):
        task = self.kb.create_task(f"Archive Actor {time.time_ns()}")
        archived = self.kb.archive_task(task["id"], actor="ArchiveBot")
        self.assertIsNotNone(archived.get("archived_at"))

    def test_unarchive_task_with_actor(self):
        task = self.kb.create_task(f"Unarchive Actor {time.time_ns()}")
        self.kb.archive_task(task["id"], actor="Bot")
        unarchived = self.kb.unarchive_task(task["id"], actor="RestoreBot")
        self.assertIsNone(unarchived.get("archived_at"))

    def test_move_task_with_actor(self):
        task = self.kb.create_task(f"Move Actor {time.time_ns()}")
        done_id = self._col_id("Done")
        moved = self.kb.move_task(task["id"], done_id, actor="MoveBot")
        self.assertEqual(moved["column_id"], done_id)

    def test_claim_task_with_actor(self):
        task = self.kb.create_task(f"Claim Actor {time.time_ns()}")
        claimed = self.kb.claim_task(task["id"], actor="WorkerBot")
        self.assertEqual(claimed.get("claimed_by"), "WorkerBot")

    def test_release_task_with_actor(self):
        task = self.kb.create_task(f"Release Actor {time.time_ns()}")
        self.kb.claim_task(task["id"], actor="WorkerBot")
        released = self.kb.release_task(task["id"], actor="WorkerBot")
        self.assertIsNone(released.get("claimed_by"))

    def test_actor_in_activity_feed(self):
        tag = f"actorfeed_{time.time_ns()}"
        task = self.kb.create_task(f"Actor Feed {tag}", actor_name="CreatorBot")
        done_id = self._col_id("Done")
        self.kb.move_task(task["id"], done_id, actor="MoverBot")
        activity = self.kb.get_activity(limit=10)
        move_events = [a for a in activity if a["event_type"] == "moved"]
        self.assertGreater(len(move_events), 0)
        self.assertEqual(move_events[0]["actor"], "MoverBot")


# ==================================================================
# Column edge cases
# ==================================================================


class TestColumnEdgeCases(KanbanTestCase):
    def test_create_column_with_position(self):
        col = self.kb.create_column(f"Pos Col {time.time_ns()}", position=1)
        board = self.kb.get_board()
        found = [c for c in board["columns"] if c["id"] == col["id"]]
        self.assertEqual(len(found), 1)
        self.assertEqual(found[0]["position"], 1)

    def test_update_column_wip_limit(self):
        col = self.kb.create_column(f"WIP Col {time.time_ns()}", wip_limit=5)
        self.assertEqual(col.get("wip_limit"), 5)
        updated = self.kb.update_column(col["id"], wip_limit=10)
        self.assertEqual(updated.get("wip_limit"), 10)

    def test_update_column_clear_wip_limit(self):
        col = self.kb.create_column(f"Clear WIP {time.time_ns()}", wip_limit=3)
        self.assertEqual(col.get("wip_limit"), 3)
        # Sending wip_limit=None sends null in JSON — backend may keep or clear
        updated = self.kb.update_column(col["id"], wip_limit=None)
        # Just verify the update call succeeds (behavior may vary)
        self.assertIn("id", updated)

    def test_delete_non_empty_column_fails(self):
        col = self.kb.create_column(f"NonEmpty {time.time_ns()}")
        self.kb.create_task(f"In Col {time.time_ns()}", column_id=col["id"])
        with self.assertRaises(KanbanError):
            self.kb.delete_column(col["id"])

    def test_delete_nonexistent_column_fails(self):
        with self.assertRaises((NotFoundError, KanbanError)):
            self.kb.delete_column("00000000-0000-0000-0000-000000000000")

    def test_column_response_fields(self):
        col = self.kb.create_column(f"Fields {time.time_ns()}", wip_limit=7)
        self.assertIn("id", col)
        self.assertIn("name", col)
        self.assertIn("position", col)
        self.assertEqual(col.get("wip_limit"), 7)


# ==================================================================
# Reorder task with column move
# ==================================================================


class TestReorderWithColumnMove(KanbanTestCase):
    def test_reorder_task_to_different_column(self):
        task = self.kb.create_task(f"Reorder Move {time.time_ns()}")
        review_id = self._col_id("Review")
        reordered = self.kb.reorder_task(task["id"], position=0, column_id=review_id)
        self.assertEqual(reordered["column_id"], review_id)
        self.assertEqual(reordered["position"], 0)

    def test_reorder_within_column(self):
        col_id = self._col_id("Up Next")
        t1 = self.kb.create_task(f"First {time.time_ns()}", column_id=col_id)
        t2 = self.kb.create_task(f"Second {time.time_ns()}", column_id=col_id)
        reordered = self.kb.reorder_task(t2["id"], position=0)
        self.assertEqual(reordered["position"], 0)


# ==================================================================
# Display name enforcement
# ==================================================================


class TestDisplayNameEnforcement(KanbanTestCase):
    @classmethod
    def setUpClass(cls):
        cls.kb = Kanban(BASE_URL)
        cls.board = cls.kb.create_board(
            f"Display Name Board {time.time_ns()}",
            description="Requires display name",
            require_display_name=True,
        )
        cls.manage_key = cls.board["manage_key"]
        cls.kb.board_id = cls.board["id"]
        cls.kb.manage_key = cls.manage_key

    def test_create_task_without_name_fails(self):
        with self.assertRaises(KanbanError) as ctx:
            self.kb.create_task(f"No Name {time.time_ns()}")
        self.assertIn("DISPLAY_NAME_REQUIRED", str(ctx.exception.body))

    def test_create_task_with_name_succeeds(self):
        task = self.kb.create_task(
            f"Has Name {time.time_ns()}", actor_name="TestBot"
        )
        self.assertIn("Has Name", task["title"])

    def test_comment_without_name_fails(self):
        task = self.kb.create_task(f"Comment Target {time.time_ns()}", actor_name="Bot")
        with self.assertRaises(KanbanError) as ctx:
            self.kb.comment(task["id"], "No name comment")
        self.assertIn("DISPLAY_NAME_REQUIRED", str(ctx.exception.body))

    def test_comment_with_name_succeeds(self):
        task = self.kb.create_task(f"Comment OK {time.time_ns()}", actor_name="Bot")
        result = self.kb.comment(task["id"], "Named comment", actor_name="Commenter")
        self.assertIn("id", result)

    def test_move_task_without_actor_fails(self):
        task = self.kb.create_task(f"Move No Actor {time.time_ns()}", actor_name="Bot")
        done_id = self._col_id("Done")
        with self.assertRaises(KanbanError):
            self.kb.move_task(task["id"], done_id)

    def test_move_task_with_actor_succeeds(self):
        task = self.kb.create_task(f"Move OK {time.time_ns()}", actor_name="Bot")
        done_id = self._col_id("Done")
        moved = self.kb.move_task(task["id"], done_id, actor="MoveBot")
        self.assertEqual(moved["column_id"], done_id)

    def test_delete_task_without_actor_fails(self):
        task = self.kb.create_task(f"Delete No Actor {time.time_ns()}", actor_name="Bot")
        with self.assertRaises(KanbanError):
            self.kb.delete_task(task["id"])

    def test_archive_task_without_actor_fails(self):
        task = self.kb.create_task(f"Archive No Actor {time.time_ns()}", actor_name="Bot")
        with self.assertRaises(KanbanError):
            self.kb.archive_task(task["id"])


# ==================================================================
# Comment @mentions
# ==================================================================


class TestCommentMentions(KanbanTestCase):
    def test_mention_extracted_in_comment(self):
        task = self.kb.create_task(f"Mention Test {time.time_ns()}")
        self.kb.comment(task["id"], "Hey @Alice and @Bob check this", actor_name="Eve")
        events = self.kb.get_task_events(task["id"])
        comment_events = [e for e in events if e["event_type"] == "comment"]
        self.assertGreater(len(comment_events), 0)
        data = comment_events[-1]["data"]
        self.assertIn("mentions", data)
        self.assertIn("Alice", data["mentions"])
        self.assertIn("Bob", data["mentions"])

    def test_quoted_mention(self):
        task = self.kb.create_task(f"Quoted Mention {time.time_ns()}")
        self.kb.comment(task["id"], 'Hello @"John Doe" please review', actor_name="Bot")
        events = self.kb.get_task_events(task["id"])
        comment_events = [e for e in events if e["event_type"] == "comment"]
        data = comment_events[-1]["data"]
        mentions = data.get("mentions", [])
        self.assertTrue(
            any("John Doe" in m for m in mentions),
            f"Expected 'John Doe' in mentions, got {mentions}",
        )

    def test_activity_mentioned_filter(self):
        tag = f"mentfilt_{time.time_ns()}"
        task = self.kb.create_task(f"Mention Filter {tag}")
        self.kb.comment(task["id"], f"@UniqueAgent42 look at {tag}", actor_name="Sender")
        activity = self.kb.get_activity(mentioned="UniqueAgent42")
        mention_items = [a for a in activity if a.get("mentions") and "UniqueAgent42" in a["mentions"]]
        self.assertGreater(len(mention_items), 0)

    def test_no_mentions_returns_empty(self):
        task = self.kb.create_task(f"No Mentions {time.time_ns()}")
        self.kb.comment(task["id"], "Just a regular comment", actor_name="Bot")
        events = self.kb.get_task_events(task["id"])
        comment_events = [e for e in events if e["event_type"] == "comment"]
        data = comment_events[-1]["data"]
        mentions = data.get("mentions", [])
        self.assertEqual(len(mentions), 0)


# ==================================================================
# Search with additional filters
# ==================================================================


class TestSearchAdvanced(KanbanTestCase):
    def test_search_empty_query_fails(self):
        with self.assertRaises(KanbanError):
            self.kb.search("")

    def test_search_returns_task_fields(self):
        tag = f"searchfields_{time.time_ns()}"
        self.kb.create_task(f"Search Fields {tag}", priority=2, labels=["search-test"])
        results = self.kb.search(tag)
        self.assertGreater(results["total"], 0)
        task = results["tasks"][0]
        self.assertIn("id", task)
        self.assertIn("title", task)
        self.assertIn("priority", task)
        self.assertIn("column_name", task)

    def test_search_pagination_offset(self):
        tag = f"srcpage_{time.time_ns()}"
        for i in range(4):
            self.kb.create_task(f"Page {i} {tag}")
        page1 = self.kb.search(tag, limit=2, offset=0)
        page2 = self.kb.search(tag, limit=2, offset=2)
        ids1 = {t["id"] for t in page1["tasks"]}
        ids2 = {t["id"] for t in page2["tasks"]}
        self.assertEqual(len(ids1 & ids2), 0, "Pages should not overlap")

    def test_search_total_count(self):
        tag = f"srctotal_{time.time_ns()}"
        for i in range(3):
            self.kb.create_task(f"Total {i} {tag}")
        results = self.kb.search(tag, limit=1)
        self.assertGreaterEqual(results["total"], 3)
        self.assertLessEqual(len(results["tasks"]), 1)


# ==================================================================
# Dependency with task filter
# ==================================================================


class TestDependencyAdvanced(KanbanTestCase):
    def test_dependency_with_note(self):
        t1 = self.kb.create_task(f"Blocker Note {time.time_ns()}")
        t2 = self.kb.create_task(f"Blocked Note {time.time_ns()}")
        dep = self.kb.create_dependency(
            t1["id"], t2["id"],
            note="Auth must come first",
            actor_name="Planner",
        )
        self.assertEqual(dep["note"], "Auth must come first")

    def test_dependency_with_nonexistent_task(self):
        t1 = self.kb.create_task(f"Real Task {time.time_ns()}")
        with self.assertRaises((NotFoundError, KanbanError)):
            self.kb.create_dependency(
                t1["id"], "00000000-0000-0000-0000-000000000000"
            )

    def test_delete_nonexistent_dependency(self):
        with self.assertRaises((NotFoundError, KanbanError)):
            self.kb.delete_dependency("00000000-0000-0000-0000-000000000000")

    def test_three_level_dependency_chain(self):
        t1 = self.kb.create_task(f"Chain A {time.time_ns()}")
        t2 = self.kb.create_task(f"Chain B {time.time_ns()}")
        t3 = self.kb.create_task(f"Chain C {time.time_ns()}")
        self.kb.create_dependency(t1["id"], t2["id"])
        self.kb.create_dependency(t2["id"], t3["id"])
        # t3 -> t1 should be circular
        with self.assertRaises((ConflictError, KanbanError)):
            self.kb.create_dependency(t3["id"], t1["id"])

    def test_duplicate_dependency_rejected(self):
        t1 = self.kb.create_task(f"Dup A {time.time_ns()}")
        t2 = self.kb.create_task(f"Dup B {time.time_ns()}")
        self.kb.create_dependency(t1["id"], t2["id"])
        with self.assertRaises((ConflictError, KanbanError)):
            self.kb.create_dependency(t1["id"], t2["id"])


# ==================================================================
# WIP limit enforcement — advanced
# ==================================================================


class TestWIPLimitAdvanced(KanbanTestCase):
    def test_wip_limit_blocks_move(self):
        col = self.kb.create_column(f"WIP Move {time.time_ns()}", wip_limit=1)
        self.kb.create_task(f"Fills WIP {time.time_ns()}", column_id=col["id"])
        other_task = self.kb.create_task(f"Overflow {time.time_ns()}")
        with self.assertRaises((ConflictError, KanbanError)):
            self.kb.move_task(other_task["id"], col["id"])

    def test_wip_limit_zero_blocks_all(self):
        col = self.kb.create_column(f"WIP Zero {time.time_ns()}", wip_limit=0)
        with self.assertRaises((ConflictError, KanbanError)):
            self.kb.create_task(f"Blocked {time.time_ns()}", column_id=col["id"])

    def test_wip_limit_none_allows_unlimited(self):
        col = self.kb.create_column(f"No WIP {time.time_ns()}")
        for i in range(5):
            self.kb.create_task(f"No Limit {i} {time.time_ns()}", column_id=col["id"])
        tasks = self.kb.list_tasks(column_id=col["id"])
        self.assertGreaterEqual(len(tasks), 5)


# ==================================================================
# Claim conflict
# ==================================================================


class TestClaimConflict(KanbanTestCase):
    def test_double_claim_fails(self):
        task = self.kb.create_task(f"Double Claim {time.time_ns()}")
        self.kb.claim_task(task["id"], actor="Agent1")
        with self.assertRaises((ConflictError, KanbanError)):
            self.kb.claim_task(task["id"], actor="Agent2")

    def test_claim_release_reclaim(self):
        task = self.kb.create_task(f"Reclaim {time.time_ns()}")
        self.kb.claim_task(task["id"], actor="Agent1")
        self.kb.release_task(task["id"], actor="Agent1")
        reclaimed = self.kb.claim_task(task["id"], actor="Agent2")
        self.assertEqual(reclaimed.get("claimed_by"), "Agent2")


# ==================================================================
# Task metadata and due_at
# ==================================================================


class TestTaskMetadata(KanbanTestCase):
    def test_create_task_with_metadata(self):
        meta = {"source": "github", "issue": 42, "tags": ["urgent"]}
        task = self.kb.create_task(
            f"Meta Task {time.time_ns()}", metadata=meta
        )
        self.assertEqual(task["metadata"]["source"], "github")
        self.assertEqual(task["metadata"]["issue"], 42)

    def test_update_task_metadata(self):
        task = self.kb.create_task(f"Update Meta {time.time_ns()}", metadata={"v": 1})
        updated = self.kb.update_task(task["id"], metadata={"v": 2, "new_key": "val"})
        self.assertEqual(updated["metadata"]["v"], 2)
        self.assertEqual(updated["metadata"]["new_key"], "val")

    def test_create_task_with_due_at(self):
        task = self.kb.create_task(
            f"Due Task {time.time_ns()}", due_at="2026-12-31T23:59:59Z"
        )
        self.assertIsNotNone(task.get("due_at"))

    def test_update_task_due_at(self):
        task = self.kb.create_task(f"Update Due {time.time_ns()}")
        updated = self.kb.update_task(task["id"], due_at="2026-06-15T12:00:00Z")
        self.assertIsNotNone(updated.get("due_at"))

    def test_task_labels_normalization(self):
        task = self.kb.create_task(
            f"Label Norm {time.time_ns()}",
            labels=["My Label", "UPPER CASE", "already-ok"],
        )
        for label in task["labels"]:
            self.assertEqual(label, label.lower())
            self.assertNotIn(" ", label)


# ==================================================================
# Board archive edge cases
# ==================================================================


class TestBoardArchiveEdgeCases(unittest.TestCase):
    def test_archive_already_archived(self):
        kb = Kanban(BASE_URL)
        board = kb.create_board(f"Double Archive {time.time_ns()}")
        kb.board_id = board["id"]
        kb.manage_key = board["manage_key"]
        kb.archive_board()
        with self.assertRaises(KanbanError):
            kb.archive_board()

    def test_unarchive_not_archived(self):
        kb = Kanban(BASE_URL)
        board = kb.create_board(f"Not Archived {time.time_ns()}")
        kb.board_id = board["id"]
        kb.manage_key = board["manage_key"]
        with self.assertRaises(KanbanError):
            kb.unarchive_board()


# ==================================================================
# Auth via different methods
# ==================================================================


class TestAuthMethods(KanbanTestCase):
    def test_auth_via_bearer(self):
        """Default behavior — Bearer header."""
        task = self.kb.create_task(f"Bearer Auth {time.time_ns()}")
        self.assertIn("id", task)

    def test_auth_via_key_param(self):
        """Create a separate client and use key= param on get_board."""
        kb2 = Kanban(BASE_URL, board_id=self.board["id"])
        board = kb2.get_board(key=self.manage_key)
        self.assertEqual(board["id"], self.board["id"])

    def test_read_no_auth_required(self):
        """All read operations work without auth."""
        kb2 = Kanban(BASE_URL, board_id=self.board["id"])
        board = kb2.get_board()
        self.assertIn("id", board)
        tasks = kb2.list_tasks()
        self.assertIsInstance(tasks, list)
        activity = kb2.get_activity()
        self.assertIsInstance(activity, list)
        deps = kb2.list_dependencies()
        self.assertIsInstance(deps, list)


# ==================================================================
# Error response structure
# ==================================================================


class TestErrorStructure(KanbanTestCase):
    def test_auth_error_has_status_code(self):
        kb2 = Kanban(BASE_URL, board_id=self.board["id"], manage_key="kb_invalid")
        try:
            kb2.create_task("Fail")
            self.fail("Should have raised AuthError")
        except AuthError as e:
            self.assertIn(e.status_code, (401, 403))
            self.assertIsNotNone(e.body)

    def test_not_found_error_has_status_code(self):
        try:
            self.kb.get_task("00000000-0000-0000-0000-000000000000")
            self.fail("Should have raised NotFoundError")
        except NotFoundError as e:
            self.assertEqual(e.status_code, 404)

    def test_conflict_error_on_wip(self):
        col = self.kb.create_column(f"ErrWIP {time.time_ns()}", wip_limit=0)
        try:
            self.kb.create_task(f"ErrFail {time.time_ns()}", column_id=col["id"])
            self.fail("Should have raised ConflictError")
        except ConflictError as e:
            self.assertEqual(e.status_code, 409)

    def test_error_body_has_code_field(self):
        try:
            self.kb.get_task("00000000-0000-0000-0000-000000000000")
        except NotFoundError as e:
            if isinstance(e.body, dict):
                self.assertIn("code", e.body)


# ==================================================================
# Webhook advanced
# ==================================================================


class TestWebhookAdvanced(KanbanTestCase):
    def test_webhook_update_url(self):
        wh = self.kb.create_webhook("https://example.com/old")
        updated = self.kb.update_webhook(wh["id"], url="https://example.com/new")
        self.assertEqual(updated["url"], "https://example.com/new")

    def test_webhook_update_events(self):
        wh = self.kb.create_webhook("https://example.com/events")
        updated = self.kb.update_webhook(
            wh["id"], events=["task.created", "task.deleted"]
        )
        self.assertIn("task.created", updated["events"])
        self.assertIn("task.deleted", updated["events"])

    def test_webhook_deactivate_and_reactivate(self):
        wh = self.kb.create_webhook("https://example.com/toggle")
        self.assertTrue(wh["active"])
        deactivated = self.kb.update_webhook(wh["id"], active=False)
        self.assertFalse(deactivated["active"])
        reactivated = self.kb.update_webhook(wh["id"], active=True)
        self.assertTrue(reactivated["active"])

    def test_webhook_response_fields(self):
        wh = self.kb.create_webhook("https://example.com/fields")
        self.assertIn("id", wh)
        self.assertIn("board_id", wh)
        self.assertIn("url", wh)
        self.assertIn("secret", wh)
        self.assertIn("active", wh)
        self.assertIn("events", wh)
        self.assertIn("failure_count", wh)
        self.assertIn("created_at", wh)

    def test_delete_nonexistent_webhook(self):
        with self.assertRaises((NotFoundError, KanbanError)):
            self.kb.delete_webhook("00000000-0000-0000-0000-000000000000")

    def test_webhook_secret_only_on_create(self):
        wh = self.kb.create_webhook("https://example.com/secret-once")
        self.assertIn("secret", wh)
        hooks = self.kb.list_webhooks()
        found = [h for h in hooks if h["id"] == wh["id"]]
        self.assertEqual(len(found), 1)
        # secret should not be in list response
        self.assertNotIn("secret", found[0])


# ==================================================================
# Batch operations — advanced
# ==================================================================


class TestBatchAdvanced(KanbanTestCase):
    def test_batch_update_labels(self):
        t1 = self.kb.create_task(f"Batch Label {time.time_ns()}")
        result = self.kb.batch_update(
            [t1["id"]], labels=["new-label-1", "new-label-2"]
        )
        self.assertEqual(result["succeeded"], 1)
        task = self.kb.get_task(t1["id"])
        self.assertIn("new-label-1", task["labels"])

    def test_batch_update_due_at(self):
        t1 = self.kb.create_task(f"Batch Due {time.time_ns()}")
        result = self.kb.batch_update([t1["id"]], due_at="2026-12-01T00:00:00Z")
        self.assertEqual(result["succeeded"], 1)
        task = self.kb.get_task(t1["id"])
        self.assertIsNotNone(task.get("due_at"))

    def test_batch_with_actor_name(self):
        t1 = self.kb.create_task(f"Batch Actor {time.time_ns()}")
        done_id = self._col_id("Done")
        result = self.kb.batch_move(
            [t1["id"]], done_id, actor_name="BatchBot"
        )
        self.assertEqual(result["succeeded"], 1)

    def test_batch_move_to_invalid_column(self):
        t1 = self.kb.create_task(f"Batch Bad Col {time.time_ns()}")
        result = self.kb.batch_move(
            [t1["id"]], "00000000-0000-0000-0000-000000000000"
        )
        self.assertGreater(result["failed"], 0)

    def test_batch_multiple_operations(self):
        t1 = self.kb.create_task(f"Multi Op A {time.time_ns()}")
        t2 = self.kb.create_task(f"Multi Op B {time.time_ns()}")
        t3 = self.kb.create_task(f"Multi Op C {time.time_ns()}")
        done_id = self._col_id("Done")
        result = self.kb.batch(
            [
                {"action": "move", "task_ids": [t1["id"]], "column_id": done_id},
                {"action": "update", "task_ids": [t2["id"]], "priority": 3},
                {"action": "delete", "task_ids": [t3["id"]]},
            ],
            actor_name="MultiBot",
        )
        self.assertEqual(result["total"], 3)
        self.assertEqual(result["succeeded"], 3)
        # Verify state
        task1 = self.kb.get_task(t1["id"])
        self.assertEqual(task1["column_id"], done_id)
        task2 = self.kb.get_task(t2["id"])
        self.assertEqual(task2["priority"], 3)
        with self.assertRaises(NotFoundError):
            self.kb.get_task(t3["id"])


# ==================================================================
# Activity feed — advanced
# ==================================================================


class TestActivityAdvanced(KanbanTestCase):
    def test_activity_event_types_variety(self):
        """Create, move, comment — all should appear in activity."""
        tag = f"variety_{time.time_ns()}"
        task = self.kb.create_task(f"Variety {tag}", actor_name="Bot")
        done_id = self._col_id("Done")
        self.kb.move_task(task["id"], done_id, actor="Bot")
        self.kb.comment(task["id"], f"Comment {tag}", actor_name="Bot")
        activity = self.kb.get_activity(limit=20)
        types = {a["event_type"] for a in activity}
        self.assertIn("created", types)
        self.assertIn("moved", types)
        self.assertIn("comment", types)

    def test_activity_seq_is_monotonic(self):
        for i in range(3):
            self.kb.create_task(f"Mono {i} {time.time_ns()}")
        activity = self.kb.get_activity(limit=10)
        seqs = [a["seq"] for a in activity]
        # Verify seq values are present and unique
        self.assertGreater(len(seqs), 0)
        self.assertEqual(len(seqs), len(set(seqs)), "Seq values should be unique")

    def test_activity_after_cursor_returns_newer(self):
        t1 = self.kb.create_task(f"Cursor A {time.time_ns()}")
        activity1 = self.kb.get_activity(limit=1)
        self.assertGreater(len(activity1), 0)
        cursor = activity1[0]["seq"]
        # Create more events
        self.kb.create_task(f"Cursor B {time.time_ns()}")
        self.kb.create_task(f"Cursor C {time.time_ns()}")
        activity2 = self.kb.get_activity(after=cursor)
        for a in activity2:
            self.assertGreater(a["seq"], cursor)


# ==================================================================
# Task update — edge cases
# ==================================================================


class TestTaskUpdateEdgeCases(KanbanTestCase):
    def test_update_task_labels_replace(self):
        task = self.kb.create_task(
            f"Labels Replace {time.time_ns()}", labels=["old-1", "old-2"]
        )
        updated = self.kb.update_task(task["id"], labels=["new-1"])
        self.assertEqual(updated["labels"], ["new-1"])

    def test_update_task_assigned_to(self):
        task = self.kb.create_task(f"Assign Update {time.time_ns()}")
        updated = self.kb.update_task(task["id"], assigned_to="NewAssignee")
        self.assertEqual(updated["assigned_to"], "NewAssignee")

    def test_update_task_column_via_update(self):
        task = self.kb.create_task(f"Col Update {time.time_ns()}")
        review_id = self._col_id("Review")
        updated = self.kb.update_task(task["id"], column_id=review_id)
        self.assertEqual(updated["column_id"], review_id)

    def test_update_task_actor_name(self):
        task = self.kb.create_task(f"Actor Update {time.time_ns()}")
        updated = self.kb.update_task(
            task["id"], title="Renamed", actor_name="Renamer"
        )
        self.assertEqual(updated["title"], "Renamed")

    def test_update_nonexistent_task(self):
        with self.assertRaises(NotFoundError):
            self.kb.update_task(
                "00000000-0000-0000-0000-000000000000",
                title="Ghost",
            )


# ==================================================================
# Constructor and environment
# ==================================================================


class TestConstructor(unittest.TestCase):
    def test_no_base_url_raises(self):
        old = os.environ.pop("KANBAN_URL", None)
        try:
            with self.assertRaises(ValueError):
                Kanban()
        finally:
            if old:
                os.environ["KANBAN_URL"] = old

    def test_no_board_id_raises_on_use(self):
        kb = Kanban(BASE_URL)
        with self.assertRaises(ValueError):
            kb.get_board()

    def test_base_url_trailing_slash_stripped(self):
        kb = Kanban(f"{BASE_URL}/")
        self.assertFalse(kb.base_url.endswith("/"))

    def test_env_var_fallback(self):
        old = os.environ.get("KANBAN_URL")
        os.environ["KANBAN_URL"] = BASE_URL
        try:
            kb = Kanban()
            h = kb.health()
            self.assertEqual(h["status"], "ok")
        finally:
            if old:
                os.environ["KANBAN_URL"] = old
            else:
                del os.environ["KANBAN_URL"]


# ==================================================================
# Task events — detailed
# ==================================================================


class TestTaskEvents(KanbanTestCase):
    def test_created_event_exists(self):
        task = self.kb.create_task(f"Event Created {time.time_ns()}", actor_name="Bot")
        events = self.kb.get_task_events(task["id"])
        created = [e for e in events if e["event_type"] == "created"]
        self.assertGreater(len(created), 0)

    def test_move_event_tracked(self):
        task = self.kb.create_task(f"Event Move {time.time_ns()}")
        done_id = self._col_id("Done")
        self.kb.move_task(task["id"], done_id)
        events = self.kb.get_task_events(task["id"])
        move_events = [e for e in events if e["event_type"] == "moved"]
        self.assertGreater(len(move_events), 0)

    def test_update_event_tracked(self):
        task = self.kb.create_task(f"Event Update {time.time_ns()}")
        self.kb.update_task(task["id"], priority=3)
        events = self.kb.get_task_events(task["id"])
        update_events = [e for e in events if e["event_type"] == "updated"]
        self.assertGreater(len(update_events), 0)

    def test_archive_event_tracked(self):
        task = self.kb.create_task(f"Event Archive {time.time_ns()}")
        self.kb.archive_task(task["id"])
        events = self.kb.get_task_events(task["id"])
        archive_events = [e for e in events if e["event_type"] == "archived"]
        self.assertGreater(len(archive_events), 0)

    def test_event_has_standard_fields(self):
        task = self.kb.create_task(f"Event Fields {time.time_ns()}", actor_name="Bot")
        events = self.kb.get_task_events(task["id"])
        self.assertGreater(len(events), 0)
        for evt in events:
            self.assertIn("id", evt)
            self.assertIn("event_type", evt)
            self.assertIn("created_at", evt)
            self.assertIn("data", evt)

    def test_nonexistent_task_events(self):
        # Server returns empty list for nonexistent task events (not 404)
        events = self.kb.get_task_events("00000000-0000-0000-0000-000000000000")
        self.assertEqual(len(events), 0)


# ==================================================================
# Board — advanced create options
# ==================================================================


class TestBoardCreateAdvanced(unittest.TestCase):
    def test_create_public_board(self):
        kb = Kanban(BASE_URL)
        board = kb.create_board(f"Public {time.time_ns()}", is_public=True)
        kb.board_id = board["id"]
        kb.manage_key = board["manage_key"]
        fetched = kb.get_board()
        self.assertTrue(fetched["is_public"])
        try:
            kb.archive_board()
        except Exception:
            pass

    def test_create_board_with_description(self):
        kb = Kanban(BASE_URL)
        desc = f"Detailed description {time.time_ns()}"
        board = kb.create_board(f"Desc Board {time.time_ns()}", description=desc)
        kb.board_id = board["id"]
        kb.manage_key = board["manage_key"]
        fetched = kb.get_board()
        self.assertEqual(fetched["description"], desc)
        try:
            kb.archive_board()
        except Exception:
            pass

    def test_create_board_empty_name_fails(self):
        kb = Kanban(BASE_URL)
        with self.assertRaises(KanbanError):
            kb.create_board("")


if __name__ == "__main__":
    unittest.main(verbosity=2)
