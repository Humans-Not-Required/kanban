// HTTP integration tests using Rocket's test client.
// These test the full request→response cycle including auth guards, rate limiting, and error handling.

#[macro_use]
extern crate rocket;

use rocket::http::{ContentType, Header, Status};
use rocket::local::blocking::Client;

use std::time::Duration;

/// Build a Rocket test client with a fresh database.
/// Uses `init_db_with_path` to avoid process-global env var races in parallel tests.
fn test_client() -> Client {
    let db_path = format!("/tmp/kanban_http_test_{}.db", uuid::Uuid::new_v4());

    let db = kanban::db::init_db_with_path(&db_path).expect("DB should initialize");
    let webhook_db = kanban::db::init_webhook_db_with_path(&db_path).expect("Webhook DB should initialize");

    // High rate limit so tests don't trip over it (unless testing rate limiting specifically)
    let rate_limiter = kanban::rate_limit::RateLimiter::new(Duration::from_secs(3600), 1000);

    let rocket = rocket::build()
        .manage(db)
        .manage(rate_limiter)
        .manage(kanban::events::EventBus::with_webhooks(webhook_db))
        .mount(
            "/api/v1",
            routes![
                kanban::routes::health,
                kanban::routes::create_board,
                kanban::routes::list_boards,
                kanban::routes::get_board,
                kanban::routes::update_board,
                kanban::routes::archive_board,
                kanban::routes::unarchive_board,
                kanban::routes::create_column,
                kanban::routes::update_column,
                kanban::routes::delete_column,
                kanban::routes::reorder_columns,
                kanban::routes::create_task,
                kanban::routes::search_tasks,
                kanban::routes::list_tasks,
                kanban::routes::get_task,
                kanban::routes::update_task,
                kanban::routes::delete_task,
                kanban::routes::archive_task,
                kanban::routes::unarchive_task,
                kanban::routes::batch_tasks,
                kanban::routes::claim_task,
                kanban::routes::release_task,
                kanban::routes::move_task,
                kanban::routes::reorder_task,
                kanban::routes::get_board_activity,
                kanban::routes::get_task_events,
                kanban::routes::comment_on_task,
                kanban::routes::board_event_stream,
                kanban::routes::create_dependency,
                kanban::routes::list_dependencies,
                kanban::routes::delete_dependency,
                kanban::routes::create_webhook,
                kanban::routes::list_webhooks,
                kanban::routes::update_webhook,
                kanban::routes::delete_webhook,
                kanban::routes::openapi,
                kanban::routes::llms_txt,
            ],
        )
        .mount("/", routes![
            kanban::routes::skills_index,
            kanban::routes::skills_skill_md,
        ])
        .register("/", catchers![
            kanban::catchers::unauthorized,
            kanban::catchers::not_found,
            kanban::catchers::unprocessable,
            kanban::catchers::too_many_requests,
            kanban::catchers::internal_error,
        ]);

    Client::tracked(rocket).expect("valid rocket instance")
}

/// Helper: create a board and return (board_id, manage_key)
fn create_test_board(client: &Client, name: &str) -> (String, String) {
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(format!(
            r#"{{"name": "{}", "columns": ["To Do", "In Progress", "Done"]}}"#,
            name
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let board_id = body["id"].as_str().unwrap().to_string();
    let manage_key = body["manage_key"].as_str().unwrap().to_string();
    (board_id, manage_key)
}

// ============ Health ============

#[test]
fn test_http_health() {
    let client = test_client();
    let resp = client.get("/api/v1/health").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["status"], "ok");
}

// ============ Board CRUD ============

#[test]
fn test_http_create_board() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Test Board", "description": "A test", "columns": ["Todo", "Done"]}"#)
        .dispatch();

    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();

    assert_eq!(body["name"], "Test Board");
    assert!(body["id"].as_str().is_some());
    assert!(body["manage_key"].as_str().unwrap().starts_with("kb_"));
    assert!(body["view_url"].as_str().is_some());
    assert!(body["manage_url"].as_str().is_some());
    assert!(body["api_base"].as_str().is_some());
    assert_eq!(body["columns"].as_array().unwrap().len(), 2);
}

#[test]
fn test_http_create_board_empty_name_rejected() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "   ", "columns": []}"#)
        .dispatch();

    assert_eq!(resp.status(), Status::BadRequest);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "EMPTY_NAME");
}

#[test]
fn test_http_create_board_default_columns() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Default Cols Board", "columns": []}"#)
        .dispatch();

    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    // When columns is empty, defaults to Backlog/Up Next/In Progress/Review/Done
    assert_eq!(body["columns"].as_array().unwrap().len(), 5);
}

#[test]
fn test_http_get_board() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Get Test");

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["name"], "Get Test");
}

#[test]
fn test_http_get_board_not_found() {
    let client = test_client();
    let resp = client
        .get("/api/v1/boards/nonexistent-uuid-1234")
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

#[test]
fn test_http_list_boards_only_public() {
    let client = test_client();

    // Create a public board
    client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Public Board", "is_public": true, "columns": ["Todo"]}"#)
        .dispatch();

    // Create an unlisted board (default)
    client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Unlisted Board", "columns": ["Todo"]}"#)
        .dispatch();

    let resp = client.get("/api/v1/boards").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let boards = body.as_array().unwrap();

    // Only public boards appear in listing
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0]["name"], "Public Board");
}

// ============ Auth Guard ============

#[test]
fn test_http_write_requires_manage_key() {
    let client = test_client();
    let (board_id, _manage_key) = create_test_board(&client, "Auth Test");

    // Try to create a task WITHOUT a manage key → should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .body(r#"{"title": "Unauthorized task"}"#)
        .dispatch();

    // Should be 401 or 403
    assert!(
        resp.status() == Status::Unauthorized || resp.status() == Status::Forbidden,
        "Expected 401/403, got {}",
        resp.status()
    );

    // Verify JSON error format from catcher
    let body: serde_json::Value = resp.into_json().expect("should be JSON");
    assert!(body["error"].is_string(), "Error response should have 'error' field");
    assert!(body["message"].is_string(), "Error response should have 'message' field");
}

#[test]
fn test_http_write_with_bearer_token() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Bearer Test");

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", manage_key)))
        .body(r#"{"title": "Authorized task"}"#)
        .dispatch();

    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["title"], "Authorized task");
}

#[test]
fn test_http_write_with_x_api_key_header() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "X-API-Key Test");

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("X-API-Key", manage_key))
        .body(r#"{"title": "X-API-Key task"}"#)
        .dispatch();

    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_http_write_with_query_param_key() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Query Param Test");

    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks?key={}",
            board_id, manage_key
        ))
        .header(ContentType::JSON)
        .body(r#"{"title": "Query param task"}"#)
        .dispatch();

    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_http_write_wrong_key_rejected() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Wrong Key Test");

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", "Bearer kb_wrong_key_12345"))
        .body(r#"{"title": "Should fail"}"#)
        .dispatch();

    assert!(
        resp.status() == Status::Forbidden || resp.status() == Status::Unauthorized,
        "Wrong key should be rejected, got {}",
        resp.status()
    );
}

// ============ Tasks ============

#[test]
fn test_http_task_crud() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Task CRUD");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "My Task", "description": "Do stuff", "priority": 2, "labels": ["bug", "urgent"]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["title"], "My Task");

    // Read task (no auth needed)
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["title"], "My Task");

    // Update task
    let resp = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Updated Task", "priority": 3}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["title"], "Updated Task");

    // List tasks (no auth)
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 1);

    // Delete task
    let resp = client
        .delete(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify deleted
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch();
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 0);
}

#[test]
fn test_http_task_empty_title_and_desc_rejected() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Empty Task");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Both empty → rejected
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "   "}"#)
        .dispatch();

    assert_eq!(resp.status(), Status::BadRequest);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "EMPTY_TASK");

    // Title only → accepted
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Has title"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Description only → accepted
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"description": "Has description but no title"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["title"], "");
    assert_eq!(body["description"], "Has description but no title");

    // No title field at all, just description → accepted
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth)
        .body(r#"{"description": "Description-only task for AI"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

// ============ Move / Claim / Release ============

#[test]
fn test_http_move_task() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Move Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Get board to find column IDs
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let columns = board["columns"].as_array().unwrap();
    let todo_col = columns[0]["id"].as_str().unwrap();
    let done_col = columns[2]["id"].as_str().unwrap();

    // Create a task (goes to first column by default)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Moveable Task"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["column_id"].as_str().unwrap(), todo_col);

    // Move to Done
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/move/{}",
            board_id, task_id, done_col
        ))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let moved: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(moved["column_id"].as_str().unwrap(), done_col);
    // Moving to last column should set completed_at
    assert!(moved["completed_at"].as_str().is_some());
}

#[test]
fn test_http_claim_and_release() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Claim Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Claimable Task"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Claim
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/claim?actor=Nanook",
            board_id, task_id
        ))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["claimed_by"], "Nanook");

    // Double-claim by same agent is OK
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/claim?actor=Nanook",
            board_id, task_id
        ))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Claim by different agent should fail (conflict)
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/claim?actor=OtherAgent",
            board_id, task_id
        ))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);

    // Release
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/release",
            board_id, task_id
        ))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert!(body["claimed_by"].is_null());
}

// ============ Comments ============

#[test]
fn test_http_comments() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Comment Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Commentable Task"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Post a comment
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/comment",
            board_id, task_id
        ))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "Hello from tests!", "actor_name": "TestBot"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["event_type"], "comment");
    assert_eq!(body["actor"], "TestBot");

    // Empty comment rejected
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/comment",
            board_id, task_id
        ))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": ""}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);

    // Read events (no auth needed)
    let resp = client
        .get(format!(
            "/api/v1/boards/{}/tasks/{}/events",
            board_id, task_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let events: serde_json::Value = resp.into_json().unwrap();
    let events_arr = events.as_array().unwrap();
    // Should have at least: created + comment
    let comment_events: Vec<_> = events_arr
        .iter()
        .filter(|e| e["event_type"] == "comment")
        .collect();
    assert_eq!(comment_events.len(), 1);
}

// ============ Archive / Unarchive ============

#[test]
fn test_http_archive_board() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Archive HTTP Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Archive
    let resp = client
        .post(format!("/api/v1/boards/{}/archive", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Double-archive should conflict
    let resp = client
        .post(format!("/api/v1/boards/{}/archive", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);

    // Write to archived board should fail (409 Conflict — board is archived)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Should Fail"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);

    // Unarchive
    let resp = client
        .post(format!("/api/v1/boards/{}/unarchive", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Now writing should work again
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Post-unarchive task"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

// ============ Search ============

#[test]
fn test_http_search_tasks() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Search HTTP Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create a few tasks
    for title in ["Fix login bug", "Add search feature", "Update docs"] {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "{}"}}"#, title))
            .dispatch();
    }

    // Search for "login"
    let resp = client
        .get(format!(
            "/api/v1/boards/{}/tasks/search?q=login",
            board_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["tasks"].as_array().unwrap().len(), 1);

    // Empty query rejected
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/search?q=", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
}

// ============ Rate Limiting ============

#[test]
fn test_http_rate_limiting() {
    let db_path = format!("/tmp/kanban_http_rl_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);
    std::env::set_var("BOARD_RATE_LIMIT", "3"); // Only 3 boards/hour for this test

    let db = kanban::db::init_db().expect("DB should initialize");
    let webhook_db = kanban::db::init_webhook_db().expect("Webhook DB should initialize");
    let rate_limiter = kanban::rate_limit::RateLimiter::new(Duration::from_secs(3600), 3);

    let rocket = rocket::build()
        .manage(db)
        .manage(rate_limiter)
        .manage(kanban::events::EventBus::with_webhooks(webhook_db))
        .mount(
            "/api/v1",
            routes![
                kanban::routes::create_board,
            ],
        );

    let client = Client::tracked(rocket).expect("valid rocket instance");

    // First 3 should succeed
    for i in 0..3 {
        let resp = client
            .post("/api/v1/boards")
            .header(ContentType::JSON)
            .body(format!(r#"{{"name": "RL Board {}", "columns": []}}"#, i))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok, "Board {} should succeed", i);
    }

    // 4th should be rate limited
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "RL Board 3", "columns": []}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::TooManyRequests);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "RATE_LIMIT_EXCEEDED");
}

// ============ Column Management ============

#[test]
fn test_http_update_column_rename() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Col Rename Test");

    // Get the board to find column IDs
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();
    assert_eq!(board["columns"][0]["name"], "To Do");

    // Rename the column
    let resp = client
        .patch(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"name": "Backlog"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let col: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(col["name"], "Backlog");
    assert_eq!(col["id"], col_id);
}

#[test]
fn test_http_update_column_no_auth() {
    let client = test_client();
    let (board_id, _key) = create_test_board(&client, "Col No Auth");

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Try without auth — should fail
    let resp = client
        .patch(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(ContentType::JSON)
        .body(r#"{"name": "Nope"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[test]
fn test_http_delete_empty_column() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Col Delete Test");

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    // Board has 3 columns: To Do, In Progress, Done. Delete the middle one (no tasks).
    let col_id = board["columns"][1]["id"].as_str().unwrap();

    let resp = client
        .delete(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["deleted"], true);

    // Verify board now has 2 columns
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(board["columns"].as_array().unwrap().len(), 2);
}

#[test]
fn test_http_delete_column_with_tasks_rejected() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Col Delete Tasks");

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Add a task to the first column
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Block Delete", "column_id": "{}"}}"#,
            col_id
        ))
        .dispatch();

    // Try to delete — should fail with 409
    let resp = client
        .delete(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "COLUMN_NOT_EMPTY");
}

#[test]
fn test_http_delete_last_column_rejected() {
    let client = test_client();

    // Create a board with just 1 column
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Single Col", "columns": ["Only"]}"#)
        .dispatch();
    let body: serde_json::Value = resp.into_json().unwrap();
    let board_id = body["id"].as_str().unwrap();
    let key = body["manage_key"].as_str().unwrap();
    let col_id = body["columns"][0]["id"].as_str().unwrap();

    // Try to delete the only column — should fail with 409
    let resp = client
        .delete(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "LAST_COLUMN");
}

#[test]
fn test_http_reorder_columns() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Col Reorder Test");

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let cols = board["columns"].as_array().unwrap();
    // Original order: To Do (0), In Progress (1), Done (2)
    let id0 = cols[0]["id"].as_str().unwrap().to_string();
    let id1 = cols[1]["id"].as_str().unwrap().to_string();
    let id2 = cols[2]["id"].as_str().unwrap().to_string();

    // Reorder: Done, To Do, In Progress
    let resp = client
        .post(format!("/api/v1/boards/{}/columns/reorder", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(serde_json::json!({ "column_ids": [id2, id0, id1] }).to_string())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let reordered: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(reordered[0]["name"], "Done");
    assert_eq!(reordered[0]["position"], 0);
    assert_eq!(reordered[1]["name"], "To Do");
    assert_eq!(reordered[1]["position"], 1);
    assert_eq!(reordered[2]["name"], "In Progress");
    assert_eq!(reordered[2]["position"], 2);
}

#[test]
fn test_http_reorder_columns_wrong_count() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Col Reorder Bad");

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let cols = board["columns"].as_array().unwrap();
    let id0 = cols[0]["id"].as_str().unwrap().to_string();

    // Send only 1 of 3 column IDs
    let resp = client
        .post(format!("/api/v1/boards/{}/columns/reorder", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(serde_json::json!({ "column_ids": [id0] }).to_string())
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "INVALID_COLUMN_LIST");
}

// ============ Update Board Settings ============

#[test]
fn test_http_update_board() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Settings Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Update name and description
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"name": "Updated Name", "description": "New desc", "is_public": true}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["name"], "Updated Name");
    assert_eq!(body["description"], "New desc");
    assert_eq!(body["is_public"], true);
}

#[test]
fn test_http_quick_done_settings() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Quick Done Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Board should start with no quick_done settings
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["quick_done_column_id"], serde_json::Value::Null);
    assert_eq!(body["quick_done_auto_archive"], false);

    // Get the first column's ID
    let first_col_id = body["columns"][0]["id"].as_str().unwrap().to_string();

    // Set quick_done_column_id and auto_archive
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"quick_done_column_id": "{}", "quick_done_auto_archive": true}}"#, first_col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["quick_done_column_id"], first_col_id);
    assert_eq!(body["quick_done_auto_archive"], true);

    // Clear quick_done_column_id by sending empty string
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"quick_done_column_id": ""}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["quick_done_column_id"], serde_json::Value::Null);
    // auto_archive should still be true
    assert_eq!(body["quick_done_auto_archive"], true);

    // Invalid column ID should be rejected
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"quick_done_column_id": "nonexistent-col"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
}

#[test]
fn test_http_update_board_empty_name_rejected() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Empty Name Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"name": "  "}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
}

#[test]
fn test_http_update_board_no_auth() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "No Auth Update");

    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .body(r#"{"name": "Hacked"}"#)
        .dispatch();
    assert!(resp.status() == Status::Unauthorized || resp.status() == Status::Forbidden);
}

// ============ Task Archive / Unarchive ============

#[test]
fn test_http_task_archive_unarchive() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Archive Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Get first column
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create a task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(serde_json::json!({"title": "Archivable", "column_id": col_id}).to_string())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();
    assert!(task["archived_at"].is_null());

    // Archive it
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/archive", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let archived: serde_json::Value = resp.into_json().unwrap();
    assert!(archived["archived_at"].is_string());

    // Archived tasks should be hidden from default list
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert!(tasks.iter().all(|t| t["id"] != task_id));

    // But visible with archived=true
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?archived=true", board_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert!(tasks.iter().any(|t| t["id"] == task_id));

    // Unarchive it
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/unarchive", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let unarchived: serde_json::Value = resp.into_json().unwrap();
    assert!(unarchived["archived_at"].is_null());

    // Now visible in default list again
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert!(tasks.iter().any(|t| t["id"] == task_id));
}

#[test]
fn test_http_task_archive_no_auth() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Archive NoAuth");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(serde_json::json!({"title": "NoAuth Archive", "column_id": col_id}).to_string())
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Try archive without auth
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/archive", board_id, task_id))
        .dispatch();
    assert!(resp.status() == Status::Unauthorized || resp.status() == Status::Forbidden);
}

#[test]
fn test_http_board_activity_feed() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Activity Feed Test");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create a task (generates a task.created event)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(serde_json::json!({"title": "Activity Task", "column_id": col_id, "actor_name": "TestBot"}).to_string())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Add a comment (generates a task.comment event)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(serde_json::json!({"message": "Test comment", "actor_name": "TestBot"}).to_string())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Fetch activity feed — should have at least 2 events
    let resp = client
        .get(format!("/api/v1/boards/{}/activity", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert!(activity.len() >= 2, "Expected at least 2 events, got {}", activity.len());

    // Should contain both event types
    let types: Vec<&str> = activity.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"comment"), "Should have comment event");
    assert!(types.contains(&"created"), "Should have created event");

    // All events should reference our task
    for event in &activity {
        assert_eq!(event["task_title"], "Activity Task");
        assert!(!event["task_id"].as_str().unwrap().is_empty());
    }

    // --- Enrichment checks ---
    // Created events should have a task snapshot
    let created_event = activity.iter().find(|e| e["event_type"] == "created").unwrap();
    assert!(created_event.get("task").is_some(), "Created event should have task snapshot");
    let task_snapshot = &created_event["task"];
    assert_eq!(task_snapshot["title"], "Activity Task");
    assert_eq!(task_snapshot["id"], task_id);
    assert!(!task_snapshot["column_id"].as_str().unwrap().is_empty());
    // Created events should NOT have recent_comments
    assert!(created_event.get("recent_comments").is_none(), "Created event should not have recent_comments");

    // Comment events should have both task snapshot and recent_comments
    let comment_event = activity.iter().find(|e| e["event_type"] == "comment").unwrap();
    assert!(comment_event.get("task").is_some(), "Comment event should have task snapshot");
    assert_eq!(comment_event["task"]["title"], "Activity Task");
    let recent = comment_event["recent_comments"].as_array().unwrap();
    assert!(!recent.is_empty(), "Comment event should have recent_comments");
    assert_eq!(recent[0]["message"], "Test comment");
    assert_eq!(recent[0]["actor"], "TestBot");

    // Move the task (generates a moved event) — should NOT be enriched
    let second_col_id = board["columns"][1]["id"].as_str().unwrap();
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}", board_id, task_id, second_col_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Re-fetch activity — moved events should stay lean
    let resp = client
        .get(format!("/api/v1/boards/{}/activity", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: Vec<serde_json::Value> = resp.into_json().unwrap();
    let moved_event = activity.iter().find(|e| e["event_type"] == "moved").unwrap();
    assert!(moved_event.get("task").is_none(), "Moved event should NOT have task snapshot");
    assert!(moved_event.get("recent_comments").is_none(), "Moved event should NOT have recent_comments");

    // Test since filter — use a future timestamp to get 0 results
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?since=2099-01-01T00:00:00", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(activity.len(), 0);

    // Test limit parameter
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?limit=1", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(activity.len(), 1);

    // --- Seq cursor pagination tests ---
    // All events should have a seq field (monotonic integer)
    let resp = client
        .get(format!("/api/v1/boards/{}/activity", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: Vec<serde_json::Value> = resp.into_json().unwrap();
    for event in &activity {
        assert!(event.get("seq").is_some(), "Event should have seq field");
        assert!(event["seq"].as_i64().unwrap() > 0, "seq should be positive");
    }

    // Test after= cursor — use seq 0 to get all events
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?after=0", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let all_after_0: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(all_after_0.len(), activity.len(), "after=0 should return all events");

    // after= results should be ordered by seq ASC (oldest first)
    let seqs: Vec<i64> = all_after_0.iter().map(|e| e["seq"].as_i64().unwrap()).collect();
    for i in 1..seqs.len() {
        assert!(seqs[i] > seqs[i-1], "after= results should be ordered by seq ASC, got {:?}", seqs);
    }

    // Test after= with a specific seq — should return only events after that seq
    let mid_seq = seqs[seqs.len() / 2];
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?after={}", board_id, mid_seq))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let partial: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert!(partial.len() < all_after_0.len(), "after=mid should return fewer events");
    for event in &partial {
        assert!(event["seq"].as_i64().unwrap() > mid_seq, "All events should have seq > {}", mid_seq);
    }

    // Test after= with a very high seq — should return 0 events
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?after=999999", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let empty: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(empty.len(), 0, "after=999999 should return no events");
}

// ============ Quick Reassign Settings ============

#[test]
fn test_http_quick_reassign_settings() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Quick Reassign Test");
    let auth = Header::new("Authorization", format!("Bearer {}", key));

    // Get board to find column IDs
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let first_col_id = board["columns"][0]["id"].as_str().unwrap();

    // Initially null
    assert!(board["quick_reassign_column_id"].is_null());
    assert!(board["quick_reassign_to"].is_null());

    // Set quick reassign settings
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"quick_reassign_column_id": "{}", "quick_reassign_to": "Jordan"}}"#, first_col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let board: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(board["quick_reassign_column_id"], first_col_id);
    assert_eq!(board["quick_reassign_to"], "Jordan");

    // Clear with empty strings
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"quick_reassign_column_id": "", "quick_reassign_to": ""}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let board: serde_json::Value = resp.into_json().unwrap();
    assert!(board["quick_reassign_column_id"].is_null());
    assert!(board["quick_reassign_to"].is_null());

    // Invalid column ID should be rejected
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"quick_reassign_column_id": "nonexistent-col"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "INVALID_COLUMN");
}

// ============ Require Display Name ============

#[test]
fn test_http_require_display_name() {
    let client = test_client();

    // Create board with require_display_name enabled
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Named Board", "require_display_name": true}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let board_id = body["id"].as_str().unwrap().to_string();
    let manage_key = body["manage_key"].as_str().unwrap().to_string();
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Verify board setting is returned
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(board["require_display_name"], true);

    // Creating a task without actor_name should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Anonymous Task"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // Creating a task WITH actor_name should succeed
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Named Task", "actor_name": "TestBot"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Commenting without actor_name should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "Anonymous comment"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // Commenting WITH actor_name should succeed
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "Named comment", "actor_name": "TestBot"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Toggling setting off should allow anonymous again
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"require_display_name": false}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let board: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(board["require_display_name"], false);

    // Now anonymous task creation should work
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Anonymous OK Now"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_http_comment_mentions() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Mentions Test");
    let auth = Header::new("Authorization", format!("Bearer {}", key));

    // Create a task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Test mentions"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Post a comment with @mentions
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "Hey @Jordan and @Nanook, please review this", "actor_name": "TestBot"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Post a comment without mentions
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "No mentions here", "actor_name": "TestBot"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Check activity — should show mentions on first comment
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?limit=50", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let items: Vec<serde_json::Value> = resp.into_json().unwrap();
    let comment_events: Vec<&serde_json::Value> = items.iter()
        .filter(|i| i["event_type"] == "comment")
        .collect();
    assert_eq!(comment_events.len(), 2);

    // Find the comment with mentions (check data.mentions)
    let with_mentions = comment_events.iter()
        .find(|e| e["data"]["mentions"].is_array())
        .expect("Should have a comment with mentions");
    let mentions = with_mentions["mentions"].as_array()
        .expect("Top-level mentions field should exist");
    assert_eq!(mentions.len(), 2);
    assert!(mentions.iter().any(|m| m == "Jordan"));
    assert!(mentions.iter().any(|m| m == "Nanook"));

    // The other comment should not have mentions
    let without_mentions = comment_events.iter()
        .find(|e| !e["data"]["mentions"].is_array())
        .expect("Should have a comment without mentions");
    assert!(without_mentions["mentions"].is_null());

    // Filter activity by ?mentioned=Jordan — should return only relevant events
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?mentioned=Jordan", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let items: Vec<serde_json::Value> = resp.into_json().unwrap();
    // Should have at least the comment that mentions Jordan
    assert!(items.iter().any(|i| i["event_type"] == "comment" && i["data"]["mentions"].is_array()));

    // Filter by ?mentioned=nobody — should return no comment mentions but may return actor-matched events
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?mentioned=nobody", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let items: Vec<serde_json::Value> = resp.into_json().unwrap();
    let mention_comments: Vec<&serde_json::Value> = items.iter()
        .filter(|i| i["event_type"] == "comment" && i["data"]["mentions"].is_array())
        .collect();
    assert_eq!(mention_comments.len(), 0);
}

#[test]
fn test_mention_extraction_quoted() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Quoted Mentions");
    let auth = Header::new("Authorization", format!("Bearer {}", key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Quoted mention test"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Post comment with quoted mention
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "cc @\"Team Lead\" and @dev-bot", "actor_name": "Tester"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    let resp = client
        .get(format!("/api/v1/boards/{}/activity?limit=10", board_id))
        .dispatch();
    let items: Vec<serde_json::Value> = resp.into_json().unwrap();
    let comment = items.iter()
        .find(|i| i["event_type"] == "comment" && i["data"]["mentions"].is_array())
        .expect("Should have comment with mentions");
    let mentions = comment["mentions"].as_array().unwrap();
    assert_eq!(mentions.len(), 2);
    assert!(mentions.iter().any(|m| m == "Team Lead"));
    assert!(mentions.iter().any(|m| m == "dev-bot"));
}

#[test]
fn test_http_require_display_name_all_endpoints() {
    let client = test_client();

    // Create board with require_display_name enabled
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Display Name Audit", "require_display_name": true}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let board_id = body["id"].as_str().unwrap().to_string();
    let manage_key = body["manage_key"].as_str().unwrap().to_string();
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Get column ID for moves
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let columns = board["columns"].as_array().unwrap();
    let _col_id = columns[0]["id"].as_str().unwrap().to_string();
    let col2_id = columns[1]["id"].as_str().unwrap().to_string();

    // Create a task WITH actor_name (should succeed)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Test Task", "actor_name": "TestBot"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap().to_string();

    // UPDATE task without actor_name → should fail
    let resp = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Updated Title"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // UPDATE task with actor_name → should succeed
    let resp = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Updated Title", "actor_name": "TestBot"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // MOVE task without actor → should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}", board_id, task_id, col2_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // MOVE task with actor → should succeed
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}?actor=TestBot", board_id, task_id, col2_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // CLAIM task without agent → should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // CLAIM task with agent → should succeed
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=TestBot", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // RELEASE task without actor → should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/release", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // RELEASE task with actor → should succeed
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/release?actor=TestBot", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // ARCHIVE task without actor → should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/archive", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // ARCHIVE task with actor → should succeed
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/archive?actor=TestBot", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // UNARCHIVE task without actor → should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/unarchive", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // UNARCHIVE task with actor → should succeed
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/unarchive?actor=TestBot", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // DELETE task without actor → should fail
    let resp = client
        .delete(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "DISPLAY_NAME_REQUIRED");

    // DELETE task with actor → should succeed
    let resp = client
        .delete(format!("/api/v1/boards/{}/tasks/{}?actor=TestBot", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_http_list_tasks_updated_before_filter() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Stale Filter");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create two tasks
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Task A", "priority": 1}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Task B", "priority": 2}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Without filter → both tasks returned
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 2);

    // With updated_before far in the future → both tasks returned
    let resp = client
        .get(format!(
            "/api/v1/boards/{}/tasks?updated_before=2099-12-31T23:59:59",
            board_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 2);

    // With updated_before far in the past → no tasks returned
    let resp = client
        .get(format!(
            "/api/v1/boards/{}/tasks?updated_before=2000-01-01T00:00:00",
            board_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 0);
}

// ============ Stale Query Parameter ============

#[test]
fn test_http_list_tasks_stale_filter() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Stale Filter Minutes");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create a task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Fresh Task", "priority": 1}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // stale=1 (1 minute) — task was just created, so it's NOT stale yet
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?stale=1", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 0, "freshly created task should not be stale");

    // stale=0 should return error (must be positive)
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?stale=0", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "INVALID_STALE");

    // stale=-5 should return error
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?stale=-5", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);

    // stale=999999 (tasks older than 999999 min) — fresh task is NOT that old
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?stale=999999", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 0, "fresh task should not be stale even with large window");

    // Verify without stale filter — task is there
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 1, "task exists without stale filter");
}

// ============ Reorder & Batch Actor Attribution ============

#[test]
fn test_http_reorder_and_batch_actor_attribution() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Actor Attribution");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create a task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Reorder Me", "actor_name": "TestUser"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Get the column IDs
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Reorder with actor param
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/reorder?actor=ReorderBot",
            board_id, task_id
        ))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"position": 0, "column_id": "{}"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Check activity for reorder event with correct actor
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?limit=10", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: serde_json::Value = resp.into_json().unwrap();
    let events = activity.as_array().unwrap();
    let reorder_event = events.iter().find(|e| e["event_type"] == "reordered");
    assert!(reorder_event.is_some(), "Should have a reordered event");
    assert_eq!(reorder_event.unwrap()["actor"], "ReorderBot");

    // Create another task for batch test
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Batch Me", "actor_name": "TestUser"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task2: serde_json::Value = resp.into_json().unwrap();
    let task2_id = task2["id"].as_str().unwrap();

    // Batch update with actor
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"actor_name": "BatchBot", "operations": [{{"action": "update", "task_ids": ["{}"], "priority": 3}}]}}"#,
            task2_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Check activity for batch update event with correct actor
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?limit=20", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: serde_json::Value = resp.into_json().unwrap();
    let events = activity.as_array().unwrap();
    let batch_update_event = events.iter().find(|e| {
        e["event_type"] == "updated" && e["actor"] == "BatchBot"
    });
    assert!(batch_update_event.is_some(), "Should have a batch updated event with BatchBot actor");

    // Reorder without actor param → defaults to "anonymous"
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/reorder",
            board_id, task_id
        ))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"position": 1, "column_id": "{}"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Batch without actor → defaults to "batch"
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"operations": [{{"action": "update", "task_ids": ["{}"], "priority": 1}}]}}"#,
            task2_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify activity has both defaults
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?limit=30", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: serde_json::Value = resp.into_json().unwrap();
    let events = activity.as_array().unwrap();
    let anon_reorder = events.iter().find(|e| e["event_type"] == "reordered" && e["actor"] == "anonymous");
    assert!(anon_reorder.is_some(), "Reorder without actor should default to anonymous");
    let batch_default = events.iter().find(|e| e["event_type"] == "updated" && e["actor"] == "batch");
    assert!(batch_default.is_some(), "Batch without actor should default to batch");
}

// ============ API Discovery Endpoints ============

#[test]
fn test_http_openapi_json() {
    let client = test_client();
    let resp = client.get("/api/v1/openapi.json").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    // Verify it's a valid OpenAPI spec
    assert_eq!(body["openapi"].as_str().unwrap_or(""), "3.0.3");
    assert!(body["info"].is_object());
    assert!(body["paths"].is_object());
}

#[test]
fn test_http_llms_txt() {
    let client = test_client();
    let resp = client.get("/api/v1/llms.txt").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body = resp.into_string().unwrap();
    assert!(body.contains("Kanban"), "llms.txt should mention Kanban");
    assert!(body.contains("/api/v1"), "llms.txt should reference API paths");
}

// ============ Single Task GET ============

#[test]
fn test_http_get_single_task() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Single Task Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Get columns to find first column ID
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create a task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"title": "Test Task", "description": "A description", "column_id": "{}", "priority": 2, "labels": ["bug", "urgent"], "actor_name": "Tester"}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // GET single task
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let fetched: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(fetched["title"], "Test Task");
    assert_eq!(fetched["description"], "A description");
    assert_eq!(fetched["priority"], 2);
    assert_eq!(fetched["created_by"], "Tester");
}

#[test]
fn test_http_get_single_task_not_found() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Task Not Found Board");

    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/nonexistent-id", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

// ============ Task Events (Activity History) ============

#[test]
fn test_http_task_events() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Task Events Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Get columns
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();
    let col2_id = board["columns"][1]["id"].as_str().unwrap();

    // Create a task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"title": "Events Task", "column_id": "{}", "actor_name": "Creator"}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Move the task to generate an event
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}?actor=Mover", board_id, task_id, col2_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Add a comment to generate another event
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "A test comment", "actor_name": "Commenter"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // GET task events
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/{}/events", board_id, task_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let events: serde_json::Value = resp.into_json().unwrap();
    let events_arr = events.as_array().unwrap();

    // Should have at least 3 events: created, moved, comment
    assert!(events_arr.len() >= 3, "Expected at least 3 events, got {}", events_arr.len());

    // Verify event types
    let event_types: Vec<&str> = events_arr.iter()
        .map(|e| e["event_type"].as_str().unwrap_or(""))
        .collect();
    assert!(event_types.contains(&"created"), "Should have 'created' event");
    assert!(event_types.contains(&"moved"), "Should have 'moved' event");
    assert!(event_types.contains(&"comment"), "Should have 'comment' event");
}

// ============ Column Creation ============

#[test]
fn test_http_create_column() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Column Create Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Get initial column count
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let initial_count = board["columns"].as_array().unwrap().len();

    // Create a new column
    let resp = client
        .post(format!("/api/v1/boards/{}/columns", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"name": "Testing", "wip_limit": 5}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let col: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(col["name"], "Testing");
    assert_eq!(col["wip_limit"], 5);

    // Verify column count increased
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(board["columns"].as_array().unwrap().len(), initial_count + 1);
}

#[test]
fn test_http_create_column_no_auth() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Column No Auth Board");

    let resp = client
        .post(format!("/api/v1/boards/{}/columns", board_id))
        .header(ContentType::JSON)
        .body(r#"{"name": "Unauthorized Column"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

// ============ Dependency Deletion ============

#[test]
fn test_http_delete_dependency() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Dep Delete Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Get first column
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create two tasks
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Blocker", "column_id": "{}", "actor_name": "Tester"}}"#, col_id))
        .dispatch();
    let task1: serde_json::Value = resp.into_json().unwrap();
    let task1_id = task1["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Blocked", "column_id": "{}", "actor_name": "Tester"}}"#, col_id))
        .dispatch();
    let task2: serde_json::Value = resp.into_json().unwrap();
    let task2_id = task2["id"].as_str().unwrap();

    // Create a dependency
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#,
            task1_id, task2_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let dep: serde_json::Value = resp.into_json().unwrap();
    let dep_id = dep["id"].as_str().unwrap();

    // Verify dependency exists
    let resp = client
        .get(format!("/api/v1/boards/{}/dependencies", board_id))
        .dispatch();
    let deps: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(deps.as_array().unwrap().len(), 1);

    // Delete the dependency
    let resp = client
        .delete(format!("/api/v1/boards/{}/dependencies/{}", board_id, dep_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify it's gone
    let resp = client
        .get(format!("/api/v1/boards/{}/dependencies", board_id))
        .dispatch();
    let deps: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(deps.as_array().unwrap().len(), 0);
}

// ============ Task Query Filters ============

#[test]
fn test_http_list_tasks_filter_by_column() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Filter Column Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col1_id = board["columns"][0]["id"].as_str().unwrap();
    let col2_id = board["columns"][1]["id"].as_str().unwrap();

    // Create tasks in different columns
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Col1 Task", "column_id": "{}", "actor_name": "T"}}"#, col1_id))
        .dispatch();
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Col2 Task", "column_id": "{}", "actor_name": "T"}}"#, col2_id))
        .dispatch();

    // Filter by column 1
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?column={}", board_id, col1_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Col1 Task");

    // Filter by column 2
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?column={}", board_id, col2_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Col2 Task");
}

#[test]
fn test_http_list_tasks_filter_by_priority() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Filter Priority Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create tasks with different priorities
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Low", "column_id": "{}", "priority": 0, "actor_name": "T"}}"#, col_id))
        .dispatch();
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "High", "column_id": "{}", "priority": 2, "actor_name": "T"}}"#, col_id))
        .dispatch();
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Critical", "column_id": "{}", "priority": 3, "actor_name": "T"}}"#, col_id))
        .dispatch();

    // priority filter returns tasks with priority >= value
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?priority=2", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 2);
    // Should be sorted by priority DESC
    assert_eq!(tasks[0]["title"], "Critical");
    assert_eq!(tasks[1]["title"], "High");
}

#[test]
fn test_http_list_tasks_filter_by_label() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Filter Label Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Bug Task", "column_id": "{}", "labels": ["bug", "urgent"], "actor_name": "T"}}"#, col_id))
        .dispatch();
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Feature Task", "column_id": "{}", "labels": ["feature"], "actor_name": "T"}}"#, col_id))
        .dispatch();

    // Filter by label
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?label=bug", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Bug Task");

    // Filter by a label that doesn't match
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?label=docs", board_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 0);
}

#[test]
fn test_http_list_tasks_filter_by_assigned() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Filter Assigned Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Alice Task", "column_id": "{}", "assigned_to": "Alice", "actor_name": "T"}}"#, col_id))
        .dispatch();
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Bob Task", "column_id": "{}", "assigned_to": "Bob", "actor_name": "T"}}"#, col_id))
        .dispatch();

    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?assigned=Alice", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Alice Task");
}

#[test]
fn test_http_list_tasks_filter_archived() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Filter Archived Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create and archive a task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Active Task", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Archived Task", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let archived_task_id = task["id"].as_str().unwrap();

    client
        .post(format!("/api/v1/boards/{}/tasks/{}/archive?actor=T", board_id, archived_task_id))
        .header(auth.clone())
        .dispatch();

    // Default listing excludes archived
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Active Task");

    // Explicitly request archived tasks
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?archived=true", board_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Archived Task");
}

#[test]
fn test_http_list_tasks_limit_and_offset() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Limit Offset Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create 5 tasks
    for i in 1..=5 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "Task {}", "column_id": "{}", "actor_name": "T"}}"#, i, col_id))
            .dispatch();
    }

    // Limit to 2
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?limit=2", board_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 2);

    // Offset by 3, should get 2 remaining
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?offset=3", board_id))
        .dispatch();
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 2);
}

#[test]
fn test_http_list_tasks_filter_by_claimed() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Filter Claimed Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Claimed Task", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Unclaimed Task", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();

    // Claim the first task
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=Worker", board_id, task_id))
        .header(auth.clone())
        .dispatch();

    // Filter by claimed_by
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?claimed=Worker", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Claimed Task");
    assert_eq!(tasks[0]["claimed_by"], "Worker");
}

// ============ Webhook HTTP Tests ============

#[test]
fn test_http_webhook_crud() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Webhook CRUD Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create webhook
    let resp = client
        .post(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"url": "https://example.com/hook", "events": ["task.created", "task.updated"]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let wh: serde_json::Value = resp.into_json().unwrap();
    let webhook_id = wh["id"].as_str().unwrap().to_string();
    assert_eq!(wh["url"], "https://example.com/hook");
    assert!(wh["secret"].as_str().unwrap().starts_with("whsec_"));
    assert_eq!(wh["active"], true);
    assert_eq!(wh["events"].as_array().unwrap().len(), 2);

    // List webhooks
    let resp = client
        .get(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let list: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(list.len(), 1);
    // Secret should not be returned on list
    assert!(list[0]["secret"].is_null());

    // Update webhook
    let resp = client
        .patch(format!("/api/v1/boards/{}/webhooks/{}", board_id, webhook_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"url": "https://new.example.com/hook", "active": false}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let updated: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(updated["url"], "https://new.example.com/hook");
    assert_eq!(updated["active"], false);

    // Delete webhook
    let resp = client
        .delete(format!("/api/v1/boards/{}/webhooks/{}", board_id, webhook_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify deleted
    let resp = client
        .get(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(auth.clone())
        .dispatch();
    let list: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(list.len(), 0);
}

#[test]
fn test_http_webhook_no_auth() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Webhook No Auth Board");

    // Create without auth
    let resp = client
        .post(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(ContentType::JSON)
        .body(r#"{"url": "https://example.com/hook", "events": ["task.created"]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);

    // List without auth
    let resp = client
        .get(format!("/api/v1/boards/{}/webhooks", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[test]
fn test_http_webhook_invalid_event() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Webhook Invalid Event Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"url": "https://example.com/hook", "events": ["invalid.event"]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "INVALID_EVENT_TYPE");
}

#[test]
fn test_http_webhook_empty_url() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Webhook Empty URL Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"url": "  ", "events": ["task.created"]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "EMPTY_URL");
}

#[test]
fn test_http_webhook_update_not_found() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Webhook Update NF Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .patch(format!("/api/v1/boards/{}/webhooks/nonexistent-id", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"url": "https://example.com"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

#[test]
fn test_http_webhook_delete_not_found() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Webhook Delete NF Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .delete(format!("/api/v1/boards/{}/webhooks/nonexistent-id", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

// ============ Board Archive Listing ============

#[test]
fn test_http_list_boards_include_archived() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Archive List Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Make it public so it shows in list
    client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"is_public": true}"#)
        .dispatch();

    // Archive the board
    client
        .post(format!("/api/v1/boards/{}/archive", board_id))
        .header(auth.clone())
        .dispatch();

    // Default listing should NOT include archived
    let resp = client.get("/api/v1/boards").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let boards: Vec<serde_json::Value> = resp.into_json().unwrap();
    let found = boards.iter().any(|b| b["id"].as_str() == Some(&board_id));
    assert!(!found, "Archived board should not appear in default listing");

    // With include_archived=true it should appear
    let resp = client.get("/api/v1/boards?include_archived=true").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let boards: Vec<serde_json::Value> = resp.into_json().unwrap();
    let found = boards.iter().any(|b| b["id"].as_str() == Some(&board_id));
    assert!(found, "Archived board should appear with include_archived=true");
}

// ============ Task Update Validation ============

#[test]
fn test_http_update_task_clear_both_title_desc() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Update Validation Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Has Title", "description": "Has Desc", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Try to clear both title and description — should fail
    let resp = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "", "description": "", "actor_name": "T"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let err: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(err["code"], "EMPTY_TASK");
}

#[test]
fn test_http_update_task_partial() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Update Partial Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Original", "description": "Keep this", "column_id": "{}", "priority": 1, "actor_name": "T"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Update only the title
    let resp = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Updated Title", "actor_name": "T"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let updated: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(updated["title"], "Updated Title");
    assert_eq!(updated["description"], "Keep this");
    assert_eq!(updated["priority"], 1);
}

// ============ Comment Edge Cases ============

#[test]
fn test_http_comment_empty_message() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Comment Empty Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Comment Test", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Empty comment message
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "", "actor_name": "T"}"#)
        .dispatch();
    // Should reject empty comment
    assert!(resp.status() == Status::BadRequest || resp.status() == Status::UnprocessableEntity,
        "Empty comment should be rejected, got {:?}", resp.status());
}

#[test]
fn test_http_comment_no_auth() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Comment Auth Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Auth Test", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Comment without auth — should fail since comments require manage key
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .body(r#"{"message": "Unauthorized comment", "actor_name": "Hacker"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

// ============ Activity Feed Filters ============

#[test]
fn test_http_activity_feed_with_limit() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Activity Limit Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create several tasks to generate activity
    for i in 1..=5 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "Task {}", "column_id": "{}", "actor_name": "T"}}"#, i, col_id))
            .dispatch();
    }

    // Get activity with limit
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?limit=2", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let activity: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(activity.len(), 2);

    // Each event should have a seq field
    assert!(activity[0]["seq"].is_number(), "Activity events should have seq field");
}

#[test]
fn test_http_activity_feed_cursor_pagination() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Activity Cursor Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create tasks to generate events
    for i in 1..=4 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "Cursor Task {}", "column_id": "{}", "actor_name": "T"}}"#, i, col_id))
            .dispatch();
    }

    // Get first page
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?limit=2", board_id))
        .dispatch();
    let page1: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(page1.len(), 2);

    // Use after= cursor from last event to get next page
    // Activity is newest-first by default, but after= returns ASC order
    let last_seq = page1.last().unwrap()["seq"].as_i64().unwrap();
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?after={}&limit=10", board_id, last_seq))
        .dispatch();
    let page2: Vec<serde_json::Value> = resp.into_json().unwrap();
    // Should get remaining events after the cursor
    assert!(!page2.is_empty(), "Should have events after cursor");

    // All events in page2 should have seq > last_seq
    for evt in &page2 {
        let seq = evt["seq"].as_i64().unwrap();
        assert!(seq > last_seq, "Event seq {} should be > cursor {}", seq, last_seq);
    }
}

// ============ Dependency Listing with Filter ============

#[test]
fn test_http_dependency_list_with_task_filter() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Dep Filter Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create 3 tasks
    let mut task_ids = Vec::new();
    for i in 1..=3 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "Dep Task {}", "column_id": "{}", "actor_name": "T"}}"#, i, col_id))
            .dispatch();
        let task: serde_json::Value = resp.into_json().unwrap();
        task_ids.push(task["id"].as_str().unwrap().to_string());
    }

    // Create dependencies: Task1 blocks Task2, Task1 blocks Task3
    client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, task_ids[0], task_ids[1]))
        .dispatch();
    client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, task_ids[0], task_ids[2]))
        .dispatch();

    // List all dependencies
    let resp = client
        .get(format!("/api/v1/boards/{}/dependencies", board_id))
        .dispatch();
    let deps: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(deps.len(), 2);

    // Filter by task ID — should show deps involving that task
    let resp = client
        .get(format!("/api/v1/boards/{}/dependencies?task={}", board_id, task_ids[1]))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let deps: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(deps.len(), 1);
}

// ============ Board Not Found ============

#[test]
fn test_http_tasks_board_not_found() {
    let client = test_client();

    let resp = client
        .get("/api/v1/boards/nonexistent-board/tasks")
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

// ============ Task Create with Description Only ============

#[test]
fn test_http_create_task_description_only() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Desc Only Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Create task with only description (empty title)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "", "description": "Just a description", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["description"], "Just a description");
}

// ============ Batch Operations (comprehensive) ============

#[test]
fn test_http_batch_move() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Batch Move Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col1_id = board["columns"][0]["id"].as_str().unwrap();
    let col2_id = board["columns"][1]["id"].as_str().unwrap();

    // Create 3 tasks in column 1
    let mut task_ids = Vec::new();
    for i in 0..3 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "Task {}", "column_id": "{}", "actor_name": "T"}}"#, i, col1_id))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let task: serde_json::Value = resp.into_json().unwrap();
        task_ids.push(task["id"].as_str().unwrap().to_string());
    }

    // Batch move all 3 to column 2
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"actor_name": "MoveBot", "operations": [{{"action": "move", "task_ids": ["{}","{}","{}"], "column_id": "{}"}}]}}"#,
            task_ids[0], task_ids[1], task_ids[2], col2_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["succeeded"], 1);
    assert_eq!(body["failed"], 0);
    assert_eq!(body["results"][0]["action"], "move");
    assert_eq!(body["results"][0]["affected"], 3);
    assert!(body["results"][0]["success"].as_bool().unwrap());

    // Verify tasks are in column 2
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?column={}", board_id, col2_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 3);
}

#[test]
fn test_http_batch_update() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Batch Update Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create 2 tasks
    let mut task_ids = Vec::new();
    for i in 0..2 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "Update Me {}", "actor_name": "T"}}"#, i))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let task: serde_json::Value = resp.into_json().unwrap();
        task_ids.push(task["id"].as_str().unwrap().to_string());
    }

    // Batch update priority and labels
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"actor_name": "UpdateBot", "operations": [{{"action": "update", "task_ids": ["{}","{}"], "priority": 2, "labels": ["urgent","backend"]}}]}}"#,
            task_ids[0], task_ids[1]
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["succeeded"], 1);
    assert_eq!(body["results"][0]["affected"], 2);

    // Verify tasks were updated
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, task_ids[0]))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["priority"], 2);
    let labels: Vec<String> = serde_json::from_value(task["labels"].clone()).unwrap();
    assert!(labels.contains(&"urgent".to_string()));
    assert!(labels.contains(&"backend".to_string()));
}

#[test]
fn test_http_batch_delete() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Batch Delete Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create 3 tasks
    let mut task_ids = Vec::new();
    for i in 0..3 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "Delete Me {}", "actor_name": "T"}}"#, i))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let task: serde_json::Value = resp.into_json().unwrap();
        task_ids.push(task["id"].as_str().unwrap().to_string());
    }

    // Delete first 2 via batch
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"actor_name": "DeleteBot", "operations": [{{"action": "delete", "task_ids": ["{}","{}"]}}]}}"#,
            task_ids[0], task_ids[1]
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["succeeded"], 1);
    assert_eq!(body["results"][0]["affected"], 2);

    // Verify only 1 task remains
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch();
    let tasks: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["id"].as_str().unwrap(), task_ids[2]);
}

#[test]
fn test_http_batch_empty_operations() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Batch Empty Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"operations": []}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "EMPTY_BATCH");
}

#[test]
fn test_http_batch_no_auth() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Batch No Auth Board");

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .body(r#"{"operations": [{"action": "delete", "task_ids": ["fake"]}]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[test]
fn test_http_batch_move_nonexistent_column() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Batch Bad Col Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Task", "actor_name": "T"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Move to nonexistent column
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"operations": [{{"action": "move", "task_ids": ["{}"], "column_id": "nonexistent"}}]}}"#,
            task_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["failed"], 1);
    assert_eq!(body["succeeded"], 0);
    assert!(!body["results"][0]["success"].as_bool().unwrap());
    assert!(body["results"][0]["error"].as_str().unwrap().contains("not found"));
}

#[test]
fn test_http_batch_mixed_operations() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Batch Mixed Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col2_id = board["columns"][1]["id"].as_str().unwrap();

    // Create 3 tasks
    let mut task_ids = Vec::new();
    for i in 0..3 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "Mixed {}", "actor_name": "T"}}"#, i))
            .dispatch();
        let task: serde_json::Value = resp.into_json().unwrap();
        task_ids.push(task["id"].as_str().unwrap().to_string());
    }

    // Multiple operations: move task 0, update task 1, delete task 2
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{
                "actor_name": "MixBot",
                "operations": [
                    {{"action": "move", "task_ids": ["{}"], "column_id": "{}"}},
                    {{"action": "update", "task_ids": ["{}"], "assigned_to": "alice"}},
                    {{"action": "delete", "task_ids": ["{}"]}}
                ]
            }}"#,
            task_ids[0], col2_id, task_ids[1], task_ids[2]
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["total"], 3);
    assert_eq!(body["succeeded"], 3);
    assert_eq!(body["failed"], 0);

    // Verify move
    let resp = client.get(format!("/api/v1/boards/{}/tasks/{}", board_id, task_ids[0])).dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["column_id"].as_str().unwrap(), col2_id);

    // Verify update
    let resp = client.get(format!("/api/v1/boards/{}/tasks/{}", board_id, task_ids[1])).dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["assigned_to"], "alice");

    // Verify delete
    let resp = client.get(format!("/api/v1/boards/{}/tasks/{}", board_id, task_ids[2])).dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

#[test]
fn test_http_batch_nonexistent_tasks_skipped() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Batch Skip Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col2_id = board["columns"][1]["id"].as_str().unwrap();

    // Create 1 real task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Real Task", "actor_name": "T"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let real_id = task["id"].as_str().unwrap();

    // Batch move with mix of real and fake task IDs
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"operations": [{{"action": "move", "task_ids": ["{}","fake-id-1","fake-id-2"], "column_id": "{}"}}]}}"#,
            real_id, col2_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    // Operation succeeds but only 1 task affected (fake ones silently skipped)
    assert_eq!(body["succeeded"], 1);
    assert_eq!(body["results"][0]["affected"], 1);
}

// ============ Dependencies (comprehensive) ============

#[test]
fn test_http_create_dependency() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Dep Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create 2 tasks
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Blocker Task", "actor_name": "T"}"#)
        .dispatch();
    let t1: serde_json::Value = resp.into_json().unwrap();
    let blocker_id = t1["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Blocked Task", "actor_name": "T"}"#)
        .dispatch();
    let t2: serde_json::Value = resp.into_json().unwrap();
    let blocked_id = t2["id"].as_str().unwrap();

    // Create dependency
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}", "note": "Must finish blocker first"}}"#,
            blocker_id, blocked_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let dep: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(dep["blocker_task_id"].as_str().unwrap(), blocker_id);
    assert_eq!(dep["blocked_task_id"].as_str().unwrap(), blocked_id);
    assert_eq!(dep["blocker_title"], "Blocker Task");
    assert_eq!(dep["blocked_title"], "Blocked Task");
    assert_eq!(dep["note"], "Must finish blocker first");
    assert!(!dep["blocker_completed"].as_bool().unwrap());

    // Verify via list
    let resp = client
        .get(format!("/api/v1/boards/{}/dependencies", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let deps: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert_eq!(deps.len(), 1);
}

#[test]
fn test_http_dependency_self_reference() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Self Dep Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Self Task", "actor_name": "T"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Self-dependency should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(
            r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#,
            task_id, task_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "SELF_DEPENDENCY");
}

#[test]
fn test_http_dependency_circular() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Circular Dep Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create 2 tasks
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Task A", "actor_name": "T"}"#)
        .dispatch();
    let ta: serde_json::Value = resp.into_json().unwrap();
    let a_id = ta["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Task B", "actor_name": "T"}"#)
        .dispatch();
    let tb: serde_json::Value = resp.into_json().unwrap();
    let b_id = tb["id"].as_str().unwrap();

    // A blocks B
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, a_id, b_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // B blocks A (circular) — should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, b_id, a_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "CIRCULAR_DEPENDENCY");
}

#[test]
fn test_http_dependency_circular_indirect() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Indirect Circular Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create 3 tasks: A → B → C, then try C → A
    let mut ids = Vec::new();
    for name in ["Task A", "Task B", "Task C"] {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "{}", "actor_name": "T"}}"#, name))
            .dispatch();
        let t: serde_json::Value = resp.into_json().unwrap();
        ids.push(t["id"].as_str().unwrap().to_string());
    }

    // A blocks B
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, ids[0], ids[1]))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // B blocks C
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, ids[1], ids[2]))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // C blocks A — indirect circular, should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, ids[2], ids[0]))
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "CIRCULAR_DEPENDENCY");
}

#[test]
fn test_http_dependency_duplicate() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Dup Dep Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let mut ids = Vec::new();
    for name in ["Task X", "Task Y"] {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "{}", "actor_name": "T"}}"#, name))
            .dispatch();
        let t: serde_json::Value = resp.into_json().unwrap();
        ids.push(t["id"].as_str().unwrap().to_string());
    }

    // Create dependency
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, ids[0], ids[1]))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Same dependency again — should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, ids[0], ids[1]))
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "DUPLICATE_DEPENDENCY");
}

#[test]
fn test_http_dependency_nonexistent_task() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Bad Dep Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Real Task", "actor_name": "T"}"#)
        .dispatch();
    let t: serde_json::Value = resp.into_json().unwrap();
    let real_id = t["id"].as_str().unwrap();

    // Blocker doesn't exist
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "fake-id", "blocked_task_id": "{}"}}"#, real_id))
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);

    // Blocked doesn't exist
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "fake-id"}}"#, real_id))
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

#[test]
fn test_http_dependency_no_auth() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Dep No Auth Board");

    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .body(r#"{"blocker_task_id": "a", "blocked_task_id": "b"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[test]
fn test_http_list_dependencies_empty() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Empty Dep Board");

    let resp = client
        .get(format!("/api/v1/boards/{}/dependencies", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let deps: Vec<serde_json::Value> = resp.into_json().unwrap();
    assert!(deps.is_empty());
}

// ============ Move Task Error Paths ============

#[test]
fn test_http_move_task_nonexistent_column() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Move Bad Col Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Task", "actor_name": "T"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Move endpoint uses column_id in URL path: /move/<target_column_id>
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/nonexistent-column?actor=Bot", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    // Returns 400 (column not found in this board) or 404
    assert!(resp.status() == Status::BadRequest || resp.status() == Status::NotFound,
        "Move to nonexistent column should be rejected, got {:?}", resp.status());
    let body: serde_json::Value = resp.into_json().unwrap();
    assert!(body["error"].as_str().unwrap().to_lowercase().contains("column"),
        "Error should mention column");
}

#[test]
fn test_http_move_task_no_auth() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Move No Auth Board");

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Move endpoint: /move/<target_column_id> — no auth should fail
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/fake-id/move/{}", board_id, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

// ============ Delete Task Edge Cases ============

#[test]
fn test_http_delete_task_not_found() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Delete NF Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .delete(format!("/api/v1/boards/{}/tasks/nonexistent-id?actor=Bot", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

#[test]
fn test_http_delete_task_no_auth() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Delete Auth Board");

    let resp = client
        .delete(format!("/api/v1/boards/{}/tasks/fake-id", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

// ============ Unarchive Board Standalone ============

#[test]
fn test_http_unarchive_board() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Unarchive Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Archive
    let resp = client
        .post(format!("/api/v1/boards/{}/archive", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify archived
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    assert!(board["archived"].as_bool().unwrap());

    // Unarchive
    let resp = client
        .post(format!("/api/v1/boards/{}/unarchive", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify unarchived
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    assert!(!board["archived"].as_bool().unwrap());
}

// ============ Board Custom Columns ============

#[test]
fn test_http_create_board_custom_columns() {
    let client = test_client();

    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Custom Cols", "columns": ["Ready", "Working", "Shipped"]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let columns = body["columns"].as_array().unwrap();
    assert_eq!(columns.len(), 3);
    assert_eq!(columns[0]["name"], "Ready");
    assert_eq!(columns[1]["name"], "Working");
    assert_eq!(columns[2]["name"], "Shipped");
}

// ============ Search Edge Cases ============

#[test]
fn test_http_search_empty_query() {
    let client = test_client();
    let (board_id, _) = create_test_board(&client, "Search Empty Board");

    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/search?q=", board_id))
        .dispatch();
    // Empty query should return 400 or empty results
    let status = resp.status();
    assert!(status == Status::BadRequest || status == Status::Ok);
}

#[test]
fn test_http_search_no_results() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Search NR Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Create a task with known content
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Alpha task", "actor_name": "T"}"#)
        .dispatch();

    // Search for something that doesn't match — returns SearchResponse { tasks: [] }
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/search?q=zyxwv", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["total"], 0);
    assert!(body["tasks"].as_array().unwrap().is_empty());
}

// ============ Activity Feed Mentioned Filter ============

#[test]
fn test_http_activity_feed_mentioned_filter() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Mention Filter Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Mention Task", "actor_name": "Tester"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Comment mentioning @Alice (endpoint is /comment singular)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "Hey @Alice please review this", "actor_name": "Bob"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Comment not mentioning Alice
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, task_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"message": "General comment no mentions", "actor_name": "Charlie"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Filter by mentioned=Alice
    let resp = client
        .get(format!("/api/v1/boards/{}/activity?mentioned=Alice", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let events: Vec<serde_json::Value> = resp.into_json().unwrap();
    // Should only get the comment that mentions Alice
    let comment_events: Vec<&serde_json::Value> = events.iter()
        .filter(|e| e["event_type"] == "comment")
        .collect();
    assert_eq!(comment_events.len(), 1);
    assert!(comment_events[0]["data"]["message"].as_str().unwrap().contains("@Alice"));
}

// ============ Claim/Release Error Paths ============

#[test]
fn test_http_claim_nonexistent_task() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Claim NF Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/nonexistent/claim?actor=Bot", board_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::NotFound);
}

#[test]
fn test_http_release_unclaimed_task() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Release Unclaimed Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Unclaimed Task", "actor_name": "T"}"#)
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Release a task that isn't claimed — should still succeed (idempotent)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/release?actor=Bot", board_id, task_id))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

// ============ Board Operations on Archived Board ============

#[test]
fn test_http_write_on_archived_board_rejected() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Archived Write Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Archive the board
    client
        .post(format!("/api/v1/boards/{}/archive", board_id))
        .header(auth.clone())
        .dispatch();

    // Try to create a task on archived board
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Should Fail", "actor_name": "T"}"#)
        .dispatch();
    assert!(resp.status() == Status::Forbidden || resp.status() == Status::Conflict,
        "Creating task on archived board should be rejected, got {:?}", resp.status());

    // Try batch operation on archived board
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"operations": [{"action": "delete", "task_ids": ["fake"]}]}"#)
        .dispatch();
    assert!(resp.status() == Status::Forbidden || resp.status() == Status::Conflict,
        "Batch on archived board should be rejected, got {:?}", resp.status());
}

// ============ Priority string parsing ============

#[test]
fn test_http_create_task_priority_strings() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Priority String Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    // Test "critical" string
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Critical Task", "priority": "critical", "actor_name": "T"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["priority"], 3);

    // Test "high" string
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "High Task", "priority": "high", "actor_name": "T"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["priority"], 2);

    // Test "low" string
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"title": "Low Task", "priority": "low", "actor_name": "T"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["priority"], 0);
}

// ============ Column WIP Limits ============

#[test]
fn test_http_column_wip_limit() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "WIP Board");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let board: serde_json::Value = resp.into_json().unwrap();
    let col_id = board["columns"][0]["id"].as_str().unwrap();

    // Set WIP limit to 2
    let resp = client
        .patch(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(r#"{"wip_limit": 2}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Create 2 tasks (should succeed)
    for i in 0..2 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(auth.clone())
            .body(format!(r#"{{"title": "WIP Task {}", "column_id": "{}", "actor_name": "T"}}"#, i, col_id))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok, "Task {} should succeed within WIP limit", i);
    }

    // 3rd task should be rejected (WIP limit exceeded)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth.clone())
        .body(format!(r#"{{"title": "Overflow", "column_id": "{}", "actor_name": "T"}}"#, col_id))
        .dispatch();
    // Should fail with conflict or bad request
    assert!(resp.status() == Status::Conflict || resp.status() == Status::BadRequest,
        "WIP limit should prevent 3rd task, got {:?}", resp.status());
}

// ── Well-Known Skills Discovery ──

#[test]
fn test_http_skills_index_json() {
    let client = test_client();
    let resp = client.get("/.well-known/skills/index.json").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let skills = body["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "kanban");
    assert!(skills[0]["description"].as_str().unwrap().len() > 20);
    let files = skills[0]["files"].as_array().unwrap();
    assert!(files.contains(&serde_json::json!("SKILL.md")));
}

#[test]
fn test_http_skills_skill_md() {
    let client = test_client();
    let resp = client.get("/.well-known/skills/kanban/SKILL.md").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body = resp.into_string().unwrap();
    // YAML frontmatter
    assert!(body.starts_with("---"));
    assert!(body.contains("name: kanban"));
    assert!(body.contains("description:"));
    // Content sections
    assert!(body.contains("# Kanban Integration"));
    assert!(body.contains("## Quick Start"));
    assert!(body.contains("## Auth Model"));
    assert!(body.contains("## Core Patterns"));
    assert!(body.contains("## Gotchas"));
}

#[test]
fn test_http_skills_index_name_matches_skill_md() {
    let client = test_client();

    let resp = client.get("/.well-known/skills/index.json").dispatch();
    let index: serde_json::Value = resp.into_json().unwrap();
    let skill_name = index["skills"][0]["name"].as_str().unwrap();

    let skill_url = format!("/.well-known/skills/{}/SKILL.md", skill_name);
    let resp = client.get(&skill_url).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body = resp.into_string().unwrap();
    let name_line = format!("name: {}", skill_name);
    assert!(body.contains(&name_line));
}

#[test]
fn test_http_skills_description_within_spec_limits() {
    let client = test_client();
    let resp = client.get("/.well-known/skills/index.json").dispatch();
    let body: serde_json::Value = resp.into_json().unwrap();
    let desc = body["skills"][0]["description"].as_str().unwrap();
    assert!(desc.len() <= 500, "Description too long: {} chars", desc.len());
    assert!(desc.len() >= 20, "Description too short: {} chars", desc.len());
}

#[test]
fn test_http_skills_skill_md_documents_endpoints() {
    let client = test_client();
    let resp = client.get("/.well-known/skills/kanban/SKILL.md").dispatch();
    let body = resp.into_string().unwrap();
    assert!(body.contains("POST /api/v1/boards"));
    assert!(body.contains("GET /api/v1/boards"));
    assert!(body.contains("/api/v1/health"));
    assert!(body.contains("manage_key"));
}

#[test]
fn test_http_skills_llms_txt_mentions_skills() {
    let client = test_client();
    let resp = client.get("/api/v1/llms.txt").dispatch();
    let body = resp.into_string().unwrap();
    assert!(body.contains("/.well-known/skills/index.json"));
    assert!(body.contains("/.well-known/skills/kanban/SKILL.md"));
}
