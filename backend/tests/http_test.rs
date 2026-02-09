// HTTP integration tests using Rocket's test client.
// These test the full request→response cycle including auth guards, rate limiting, and error handling.

#[macro_use]
extern crate rocket;

use rocket::http::{ContentType, Header, Status};
use rocket::local::blocking::Client;

use std::time::Duration;

/// Build a Rocket test client with a fresh database.
fn test_client() -> Client {
    let db_path = format!("/tmp/kanban_http_test_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);
    // High rate limit so tests don't trip over it (unless testing rate limiting specifically)
    std::env::set_var("BOARD_RATE_LIMIT", "1000");

    let db = kanban::db::init_db().expect("DB should initialize");
    let webhook_db = kanban::db::init_webhook_db().expect("Webhook DB should initialize");

    let rate_limit = std::env::var("BOARD_RATE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1000);
    let rate_limiter = kanban::rate_limit::RateLimiter::new(Duration::from_secs(3600), rate_limit);

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
            ],
        )
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
    // When columns is empty, defaults to Backlog/In Progress/Review/Done
    assert_eq!(body["columns"].as_array().unwrap().len(), 4);
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
fn test_http_task_empty_title_rejected() {
    let client = test_client();
    let (board_id, manage_key) = create_test_board(&client, "Empty Title");
    let auth = Header::new("Authorization", format!("Bearer {}", manage_key));

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(auth)
        .body(r#"{"title": "   "}"#)
        .dispatch();

    assert_eq!(resp.status(), Status::BadRequest);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "EMPTY_TITLE");
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
            "/api/v1/boards/{}/tasks/{}/claim?agent=Nanook",
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
            "/api/v1/boards/{}/tasks/{}/claim?agent=Nanook",
            board_id, task_id
        ))
        .header(auth.clone())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Claim by different agent should fail (conflict)
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/claim?agent=OtherAgent",
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
}
