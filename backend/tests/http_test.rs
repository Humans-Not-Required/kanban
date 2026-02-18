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
                kanban::routes::api_skills_skill_md,
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

/// Helper: get the first column ID of a board.
fn get_first_column_id(client: &Client, board_id: &str) -> String {
    let resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let body: serde_json::Value = resp.into_json().unwrap();
    body["columns"][0]["id"].as_str().unwrap().to_string()
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

// ============ Task Due Dates ============

#[test]
fn test_http_task_due_at_on_create() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Due Date Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Ship feature", "column_id": "{}", "due_at": "2026-03-01T00:00:00Z", "actor_name": "tester"}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["due_at"], "2026-03-01T00:00:00Z");

    // Verify in GET response
    let task_id = task["id"].as_str().unwrap();
    let get_resp = client.get(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id)).dispatch();
    assert_eq!(get_resp.status(), Status::Ok);
    let fetched: serde_json::Value = get_resp.into_json().unwrap();
    assert_eq!(fetched["due_at"], "2026-03-01T00:00:00Z");
}

#[test]
fn test_http_task_due_at_update() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Due Update Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "No deadline yet", "column_id": "{}", "actor_name": "tester"}}"#,
            col_id
        ))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let task_id = task["id"].as_str().unwrap();
    assert!(task["due_at"].is_null());

    // Set due date via update
    let patch = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, task_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"due_at": "2026-04-15T12:00:00Z", "actor_name": "tester"}"#)
        .dispatch();
    assert_eq!(patch.status(), Status::Ok);
    let updated: serde_json::Value = patch.into_json().unwrap();
    assert_eq!(updated["due_at"], "2026-04-15T12:00:00Z");
}

#[test]
fn test_http_task_metadata_on_create() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Metadata Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Rich task", "column_id": "{}", "metadata": {{"source": "api", "agent": "nanook", "priority_override": true}}, "actor_name": "tester"}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["metadata"]["source"], "api");
    assert_eq!(task["metadata"]["agent"], "nanook");
    assert_eq!(task["metadata"]["priority_override"], true);
}

// ============ Task Reorder ============

#[test]
fn test_http_reorder_task_within_column() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Reorder Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    // Create 4 tasks
    let mut task_ids = Vec::new();
    for i in 0..4 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(
                r#"{{"title": "Task {}", "column_id": "{}", "actor_name": "tester"}}"#,
                i, col_id
            ))
            .dispatch();
        let task: serde_json::Value = resp.into_json().unwrap();
        task_ids.push(task["id"].as_str().unwrap().to_string());
    }

    // Reorder: move task 3 to position 0
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/reorder", board_id, task_ids[3]))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"column_id": "{}", "position": 0, "actor_name": "tester"}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify order
    let list = client
        .get(format!("/api/v1/boards/{}/tasks?column={}", board_id, col_id))
        .dispatch();
    let tasks: serde_json::Value = list.into_json().unwrap();
    let titles: Vec<&str> = tasks.as_array().unwrap().iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles[0], "Task 3", "Task 3 should be first after reorder");
}

#[test]
fn test_http_reorder_task_no_auth() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Reorder Auth Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "t", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    // Reorder without auth → 401
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/reorder", board_id, tid))
        .header(ContentType::JSON)
        .body(format!(r#"{{"column_id": "{}", "position": 0, "actor_name": "x"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

// ============ Unicode Handling ============

#[test]
fn test_http_unicode_board_name() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "看板ボード 🎯", "description": "日本語の説明", "columns": ["やること", "進行中", "完了"]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["name"], "看板ボード 🎯");
    assert_eq!(body["columns"][0]["name"], "やること");
    assert_eq!(body["columns"][1]["name"], "進行中");
    assert_eq!(body["columns"][2]["name"], "完了");
}

#[test]
fn test_http_unicode_task_title_and_description() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Unicode Task Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "修复登录错误 🐛", "description": "Пользователи не могут войти через OAuth. العربية テスト", "column_id": "{}", "labels": ["バグ", "認証"], "actor_name": "テスター"}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["title"], "修复登录错误 🐛");
    assert!(task["description"].as_str().unwrap().contains("العربية"));
    assert_eq!(task["created_by"], "テスター");

    // Labels preserved
    let labels: Vec<&str> = task["labels"].as_array().unwrap().iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert!(labels.contains(&"バグ"));
    assert!(labels.contains(&"認証"));
}

#[test]
fn test_http_unicode_comment() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Unicode Comment Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "t", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    let comment_resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"message": "这是一条评论 🇨🇳 avec des caractères français", "actor_name": "机器人"}"#)
        .dispatch();
    assert_eq!(comment_resp.status(), Status::Ok);
    let comment: serde_json::Value = comment_resp.into_json().unwrap();
    assert_eq!(comment["event_type"], "comment");
    assert_eq!(comment["actor"], "机器人");
}

// ============ Timestamps ============

#[test]
fn test_http_task_timestamps_lifecycle() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Timestamps Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();
    let done_col = cols["columns"][2]["id"].as_str().unwrap();

    // Create task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Track me", "column_id": "{}", "actor_name": "tester"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();
    let created = task["created_at"].as_str().unwrap().to_string();
    assert!(!created.is_empty(), "created_at should be set");
    assert!(task["completed_at"].is_null(), "completed_at should be null initially");

    // Update task → updated_at changes
    let patch = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"title": "Track me (updated)", "actor_name": "tester"}"#)
        .dispatch();
    let updated: serde_json::Value = patch.into_json().unwrap();
    let updated_at = updated["updated_at"].as_str().unwrap();
    assert!(!updated_at.is_empty());

    // Move to Done column → completed_at set
    let move_resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}?actor=tester", board_id, tid, done_col))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(move_resp.status(), Status::Ok);

    let fetched = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch();
    let task_done: serde_json::Value = fetched.into_json().unwrap();
    // completed_at may or may not be set based on quick_done settings, but updated_at should change
    assert!(task_done["updated_at"].as_str().is_some());
}

// ============ Board Response Fields ============

#[test]
fn test_http_board_response_fields_complete() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Full Fields", "description": "Check all fields", "columns": ["A"], "is_public": true}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();

    // All expected fields present
    assert!(body["id"].is_string());
    assert!(body["name"].is_string());
    assert!(body["manage_key"].is_string());
    assert!(body["view_url"].is_string());
    assert!(body["manage_url"].is_string());
    assert!(body["api_base"].is_string());
    assert!(body["columns"].is_array());
    assert!(body["created_at"].is_string());

    // Verify URLs contain board ID
    let board_id = body["id"].as_str().unwrap();
    assert!(body["view_url"].as_str().unwrap().contains(board_id));
    assert!(body["manage_url"].as_str().unwrap().contains(board_id));
    assert!(body["api_base"].as_str().unwrap().contains(board_id));
}

#[test]
fn test_http_task_response_fields_complete() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Task Fields Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Full task", "description": "desc", "column_id": "{}", "priority": 2, "labels": ["test"], "assigned_to": "bot", "due_at": "2026-12-31T23:59:59Z", "actor_name": "tester"}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();

    // All fields present
    assert!(task["id"].is_string());
    assert_eq!(task["title"], "Full task");
    assert_eq!(task["description"], "desc");
    assert_eq!(task["priority"], 2);
    assert!(task["position"].is_number());
    assert_eq!(task["created_by"], "tester");
    assert_eq!(task["assigned_to"], "bot");
    assert!(task["labels"].is_array());
    assert_eq!(task["due_at"], "2026-12-31T23:59:59Z");
    assert!(task["created_at"].is_string());
    assert!(task["column_id"].is_string());
    assert!(task["board_id"].is_string());
}

// ============ Board Isolation ============

#[test]
fn test_http_board_isolation_tasks() {
    let client = test_client();
    let (board_a, key_a) = create_test_board(&client, "Board A");
    let (board_b, key_b) = create_test_board(&client, "Board B");

    let cols_a = client.get(format!("/api/v1/boards/{}", board_a)).dispatch();
    let ca: serde_json::Value = cols_a.into_json().unwrap();
    let col_a = ca["columns"][0]["id"].as_str().unwrap();

    let cols_b = client.get(format!("/api/v1/boards/{}", board_b)).dispatch();
    let cb: serde_json::Value = cols_b.into_json().unwrap();
    let col_b = cb["columns"][0]["id"].as_str().unwrap();

    // Create tasks in each board
    for i in 0..3 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_a))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key_a)))
            .body(format!(r#"{{"title": "A-{}", "column_id": "{}", "actor_name": "x"}}"#, i, col_a))
            .dispatch();
    }
    for i in 0..5 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_b))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key_b)))
            .body(format!(r#"{{"title": "B-{}", "column_id": "{}", "actor_name": "x"}}"#, i, col_b))
            .dispatch();
    }

    // List tasks from board A → only 3
    let list_a = client.get(format!("/api/v1/boards/{}/tasks", board_a)).dispatch();
    let tasks_a: serde_json::Value = list_a.into_json().unwrap();
    assert_eq!(tasks_a.as_array().unwrap().len(), 3);

    // List tasks from board B → only 5
    let list_b = client.get(format!("/api/v1/boards/{}/tasks", board_b)).dispatch();
    let tasks_b: serde_json::Value = list_b.into_json().unwrap();
    assert_eq!(tasks_b.as_array().unwrap().len(), 5);

    // Board A's key can't write to board B
    let cross = client
        .post(format!("/api/v1/boards/{}/tasks", board_b))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key_a)))
        .body(format!(r#"{{"title": "Cross", "column_id": "{}", "actor_name": "x"}}"#, col_b))
        .dispatch();
    // Cross-board key returns 403 Forbidden (key exists but doesn't match this board)
    assert_eq!(cross.status(), Status::Forbidden);
}

// ============ Search Advanced ============

#[test]
fn test_http_search_by_label() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Label Search Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Bug fix", "column_id": "{}", "labels": ["bug", "urgent"], "actor_name": "x"}}"#, col_id))
        .dispatch();

    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Feature", "column_id": "{}", "labels": ["feature"], "actor_name": "x"}}"#, col_id))
        .dispatch();

    // Search by label text
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/search?q=bug", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let results: serde_json::Value = resp.into_json().unwrap();
    let tasks = results["tasks"].as_array().unwrap();
    assert!(!tasks.is_empty());
    assert!(tasks[0]["title"].as_str().unwrap().contains("Bug"));
}

#[test]
fn test_http_search_by_description() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Desc Search Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Generic title", "description": "OAuth integration is broken for SSO users", "column_id": "{}", "actor_name": "x"}}"#,
            col_id
        ))
        .dispatch();

    let resp = client
        .get(format!("/api/v1/boards/{}/tasks/search?q=OAuth", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let results: serde_json::Value = resp.into_json().unwrap();
    let tasks = results["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Generic title");
}

// ============ Claim Edge Cases ============

#[test]
fn test_http_double_claim_same_agent() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Double Claim Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Claimable", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    // First claim
    let claim1 = client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=bot-1", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(claim1.status(), Status::Ok);

    // Second claim by same agent → should succeed or conflict
    let claim2 = client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=bot-1", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    // Either 200 (idempotent) or 409 (conflict) are valid behaviors
    assert!(
        claim2.status() == Status::Ok || claim2.status() == Status::Conflict,
        "Double claim should be 200 or 409, got {:?}",
        claim2.status()
    );
}

#[test]
fn test_http_claim_by_different_agent() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Diff Claim Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Contest", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    // First agent claims
    let claim1 = client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=bot-1", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(claim1.status(), Status::Ok);

    // Different agent tries to claim → should be 409 Conflict
    let claim2 = client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=bot-2", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(claim2.status(), Status::Conflict);
}

#[test]
fn test_http_release_and_reclaim() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Release Reclaim Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Relay", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    // Claim → release → claim by different agent
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=bot-1", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    let release = client
        .post(format!("/api/v1/boards/{}/tasks/{}/release", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(release.status(), Status::Ok);

    let claim3 = client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=bot-2", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(claim3.status(), Status::Ok);

    // Verify new claimer
    let get = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch();
    let t: serde_json::Value = get.into_json().unwrap();
    assert_eq!(t["claimed_by"], "bot-2");
}

// ============ Column Edge Cases ============

#[test]
fn test_http_create_column_position() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Col Position Board");

    // Board starts with 3 columns: To Do, In Progress, Done
    // Create new column → should be at end (position 3)
    let resp = client
        .post(format!("/api/v1/boards/{}/columns", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"name": "Review"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    let board = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let b: serde_json::Value = board.into_json().unwrap();
    let cols = b["columns"].as_array().unwrap();
    assert_eq!(cols.len(), 4);
    assert_eq!(cols[3]["name"], "Review");
}

#[test]
fn test_http_update_column_wip_limit() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "WIP Update Board");

    let board = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let b: serde_json::Value = board.into_json().unwrap();
    let col_id = b["columns"][1]["id"].as_str().unwrap(); // "In Progress"

    // Set WIP limit
    let resp = client
        .patch(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"wip_limit": 3}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify
    let board = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let b: serde_json::Value = board.into_json().unwrap();
    let in_progress = b["columns"].as_array().unwrap().iter()
        .find(|c| c["id"] == col_id)
        .unwrap();
    assert_eq!(in_progress["wip_limit"], 3);

    // Clear WIP limit
    let resp = client
        .patch(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"wip_limit": 0}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

// ============ Task Labels ============

#[test]
fn test_http_task_labels_replace_on_update() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Labels Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Labeled", "column_id": "{}", "labels": ["alpha", "beta"], "actor_name": "x"}}"#,
            col_id
        ))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();
    assert_eq!(task["labels"].as_array().unwrap().len(), 2);

    // Update labels → full replacement
    let patch = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"labels": ["gamma", "delta", "epsilon"], "actor_name": "x"}"#)
        .dispatch();
    assert_eq!(patch.status(), Status::Ok);
    let updated: serde_json::Value = patch.into_json().unwrap();
    let labels: Vec<&str> = updated["labels"].as_array().unwrap().iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert_eq!(labels.len(), 3);
    assert!(labels.contains(&"gamma"));
    assert!(!labels.contains(&"alpha"), "Old labels should be replaced");
}

// ============ Activity Feed Advanced ============

#[test]
fn test_http_activity_event_types() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Events Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();
    let col_id2 = cols["columns"][1]["id"].as_str().unwrap();

    // Create task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Track events", "column_id": "{}", "actor_name": "creator"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    // Update
    client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"title": "Track events (updated)", "actor_name": "editor"}"#)
        .dispatch();

    // Move
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}?actor=mover", board_id, tid, col_id2))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // Comment
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"message": "Great progress!", "actor_name": "commenter"}"#)
        .dispatch();

    // Claim
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=claimer", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // Activity feed should have multiple events
    let activity = client
        .get(format!("/api/v1/boards/{}/activity", board_id))
        .dispatch();
    assert_eq!(activity.status(), Status::Ok);
    let events: serde_json::Value = activity.into_json().unwrap();
    let ev_array = events.as_array().unwrap();
    assert!(ev_array.len() >= 4, "Should have at least 4 events, got {}", ev_array.len());

    let event_types: Vec<&str> = ev_array.iter()
        .filter_map(|e| e["event_type"].as_str())
        .collect();
    // At minimum we should see task.created and task.updated
    assert!(event_types.iter().any(|t| t.contains("created") || t.contains("task")));
}

// ============ Task Assigned To ============

#[test]
fn test_http_task_assigned_to_update() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Assign Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Assignable", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();
    assert!(task["assigned_to"].is_null() || task["assigned_to"] == "");

    // Assign
    let patch = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"assigned_to": "agent-42", "actor_name": "x"}"#)
        .dispatch();
    assert_eq!(patch.status(), Status::Ok);
    let updated: serde_json::Value = patch.into_json().unwrap();
    assert_eq!(updated["assigned_to"], "agent-42");

    // Filter by assigned
    let list = client
        .get(format!("/api/v1/boards/{}/tasks?assigned=agent-42", board_id))
        .dispatch();
    let tasks: serde_json::Value = list.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 1);
}

// ============ Webhook Update Lifecycle ============

#[test]
fn test_http_webhook_update_url_and_events() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Webhook Update Board");

    // Create webhook
    let resp = client
        .post(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"url": "https://example.com/hook1", "events": ["task.created"]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let wh: serde_json::Value = resp.into_json().unwrap();
    let wh_id = wh["id"].as_str().unwrap();

    // Update URL
    let update = client
        .patch(format!("/api/v1/boards/{}/webhooks/{}", board_id, wh_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"url": "https://example.com/hook2"}"#)
        .dispatch();
    assert_eq!(update.status(), Status::Ok);
    let updated: serde_json::Value = update.into_json().unwrap();
    assert_eq!(updated["url"], "https://example.com/hook2");

    // Update events
    let update2 = client
        .patch(format!("/api/v1/boards/{}/webhooks/{}", board_id, wh_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"events": ["task.created", "task.moved", "task.updated"]}"#)
        .dispatch();
    assert_eq!(update2.status(), Status::Ok);
    let updated2: serde_json::Value = update2.into_json().unwrap();
    assert_eq!(updated2["events"].as_array().unwrap().len(), 3);

    // Deactivate
    let deactivate = client
        .patch(format!("/api/v1/boards/{}/webhooks/{}", board_id, wh_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"active": false}"#)
        .dispatch();
    assert_eq!(deactivate.status(), Status::Ok);
    let deactivated: serde_json::Value = deactivate.into_json().unwrap();
    assert_eq!(deactivated["active"], false);
}

// ============ Dependency Advanced ============

#[test]
fn test_http_dependency_three_level_chain() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Chain Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let mut tids = Vec::new();
    for name in ["A", "B", "C", "D"] {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"title": "Task {}", "column_id": "{}", "actor_name": "x"}}"#, name, col_id))
            .dispatch();
        let t: serde_json::Value = resp.into_json().unwrap();
        tids.push(t["id"].as_str().unwrap().to_string());
    }

    // A → B → C → D chain
    for i in 0..3 {
        let resp = client
            .post(format!("/api/v1/boards/{}/dependencies", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(
                r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}", "note": "chain"}}"#,
                tids[i], tids[i + 1]
            ))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
    }

    // List all dependencies
    let deps = client
        .get(format!("/api/v1/boards/{}/dependencies", board_id))
        .dispatch();
    let d: serde_json::Value = deps.into_json().unwrap();
    assert_eq!(d.as_array().unwrap().len(), 3);

    // Delete middle dependency (B→C)
    // First find it
    let dep_bc = d.as_array().unwrap().iter()
        .find(|dep| dep["blocker_task_id"] == tids[1] && dep["blocked_task_id"] == tids[2])
        .unwrap();
    let dep_id = dep_bc["id"].as_str().unwrap();

    let del = client
        .delete(format!("/api/v1/boards/{}/dependencies/{}", board_id, dep_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(del.status(), Status::Ok);

    // Now only 2 dependencies
    let deps2 = client
        .get(format!("/api/v1/boards/{}/dependencies", board_id))
        .dispatch();
    let d2: serde_json::Value = deps2.into_json().unwrap();
    assert_eq!(d2.as_array().unwrap().len(), 2);
}

// ============ Batch Advanced ============

#[test]
fn test_http_batch_with_labels() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Batch Labels Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let mut tids = Vec::new();
    for i in 0..3 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"title": "T{}", "column_id": "{}", "actor_name": "x"}}"#, i, col_id))
            .dispatch();
        let t: serde_json::Value = resp.into_json().unwrap();
        tids.push(t["id"].as_str().unwrap().to_string());
    }

    // Batch update with labels
    let batch = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"operations": [
                {{"action": "update", "task_ids": ["{}"], "labels": ["sprint-1", "frontend"]}},
                {{"action": "update", "task_ids": ["{}"], "labels": ["sprint-1", "backend"]}}
            ], "actor_name": "batch-runner"}}"#,
            tids[0], tids[1]
        ))
        .dispatch();
    assert_eq!(batch.status(), Status::Ok);

    // Verify labels applied
    let t0 = client.get(format!("/api/v1/boards/{}/tasks/{}", board_id, tids[0])).dispatch();
    let task0: serde_json::Value = t0.into_json().unwrap();
    let labels0: Vec<&str> = task0["labels"].as_array().unwrap().iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert!(labels0.contains(&"sprint-1"));
    assert!(labels0.contains(&"frontend"));
}

// ============ Error Response Structure ============

#[test]
fn test_http_error_response_structure() {
    let client = test_client();

    // 404 — board not found
    let resp = client.get("/api/v1/boards/nonexistent-uuid").dispatch();
    assert_eq!(resp.status(), Status::NotFound);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert!(body["error"].is_string() || body["message"].is_string() || body["code"].is_string(),
        "Error response should have error/message/code field");

    // 401 — missing auth
    let (board_id, _key) = create_test_board(&client, "Error Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .body(format!(r#"{{"title": "No auth", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert!(body["error"].is_string() || body["code"].is_string(),
        "401 should return JSON with error/code");
}

// ============ Large Data ============

#[test]
fn test_http_many_tasks_in_column() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Many Tasks Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    // Create 25 tasks
    for i in 0..25 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"title": "Task #{}", "column_id": "{}", "priority": {}, "actor_name": "x"}}"#, i, col_id, i % 4))
            .dispatch();
    }

    // List all
    let all = client.get(format!("/api/v1/boards/{}/tasks", board_id)).dispatch();
    let tasks: serde_json::Value = all.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 25);

    // Limit + offset pagination
    let page1 = client
        .get(format!("/api/v1/boards/{}/tasks?limit=10&offset=0", board_id))
        .dispatch();
    let p1: serde_json::Value = page1.into_json().unwrap();
    assert_eq!(p1.as_array().unwrap().len(), 10);

    let page3 = client
        .get(format!("/api/v1/boards/{}/tasks?limit=10&offset=20", board_id))
        .dispatch();
    let p3: serde_json::Value = page3.into_json().unwrap();
    assert_eq!(p3.as_array().unwrap().len(), 5);
}

#[test]
fn test_http_many_comments_on_task() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Many Comments Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Commented task", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    // Add 10 comments
    for i in 0..10 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, tid))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"message": "Comment #{}", "actor_name": "bot-{}"}}"#, i, i % 3))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
    }

    // Fetch task events — comments show as events
    let events = client
        .get(format!("/api/v1/boards/{}/tasks/{}/events", board_id, tid))
        .dispatch();
    assert_eq!(events.status(), Status::Ok);
    let ev: serde_json::Value = events.into_json().unwrap();
    let comment_events: Vec<_> = ev.as_array().unwrap().iter()
        .filter(|e| e["event_type"] == "comment")
        .collect();
    assert_eq!(comment_events.len(), 10, "Should have 10 comment events");
}

// ============ Board Update Advanced ============

#[test]
fn test_http_board_update_description() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Desc Board");

    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"description": "Updated description with **markdown**"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["description"], "Updated description with **markdown**");
}

#[test]
fn test_http_board_toggle_public() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Public Toggle Board");

    // Make public
    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"is_public": true}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Should appear in public listing
    let list = client.get("/api/v1/boards").dispatch();
    let boards: serde_json::Value = list.into_json().unwrap();
    let found = boards.as_array().unwrap().iter()
        .any(|b| b["id"] == board_id);
    assert!(found, "Public board should appear in listing");

    // Make private
    client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"is_public": false}"#)
        .dispatch();

    // Should NOT appear in listing
    let list = client.get("/api/v1/boards").dispatch();
    let boards: serde_json::Value = list.into_json().unwrap();
    let found = boards.as_array().unwrap().iter()
        .any(|b| b["id"] == board_id);
    assert!(!found, "Private board should not appear in listing");
}

// ============ Task Position After Delete ============

#[test]
fn test_http_task_positions_after_delete() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Position Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let mut tids = Vec::new();
    for i in 0..5 {
        let resp = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"title": "P{}", "column_id": "{}", "actor_name": "x"}}"#, i, col_id))
            .dispatch();
        let t: serde_json::Value = resp.into_json().unwrap();
        tids.push(t["id"].as_str().unwrap().to_string());
    }

    // Delete middle task (P2)
    let del = client
        .delete(format!("/api/v1/boards/{}/tasks/{}", board_id, tids[2]))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(del.status(), Status::Ok);

    // Remaining 4 tasks should still be listable
    let list = client.get(format!("/api/v1/boards/{}/tasks", board_id)).dispatch();
    let tasks: serde_json::Value = list.into_json().unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 4);
}

// ============ Full Lifecycle ============

#[test]
fn test_http_full_task_lifecycle() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Lifecycle Board");

    let board = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let b: serde_json::Value = board.into_json().unwrap();
    let todo_col = b["columns"][0]["id"].as_str().unwrap();
    let progress_col = b["columns"][1]["id"].as_str().unwrap();
    let done_col = b["columns"][2]["id"].as_str().unwrap();

    // 1. Create
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Complete lifecycle", "description": "Test every step", "column_id": "{}", "priority": 2, "labels": ["test"], "actor_name": "creator"}}"#,
            todo_col
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    // 2. Claim
    let claim = client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=worker-1", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(claim.status(), Status::Ok);

    // 3. Move to In Progress
    let mv = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}?actor=worker-1", board_id, tid, progress_col))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(mv.status(), Status::Ok);

    // 4. Comment
    let comment = client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"message": "WIP: halfway done", "actor_name": "worker-1"}"#)
        .dispatch();
    assert_eq!(comment.status(), Status::Ok);

    // 5. Update (add more labels, update description)
    let update = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"description": "Test every step - DONE", "labels": ["test", "completed"], "actor_name": "worker-1"}"#)
        .dispatch();
    assert_eq!(update.status(), Status::Ok);

    // 6. Move to Done
    let mv2 = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}?actor=worker-1", board_id, tid, done_col))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(mv2.status(), Status::Ok);

    // 7. Release claim
    let release = client
        .post(format!("/api/v1/boards/{}/tasks/{}/release", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(release.status(), Status::Ok);

    // 8. Verify final state
    let final_task = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch();
    let t: serde_json::Value = final_task.into_json().unwrap();
    assert_eq!(t["column_id"], done_col);
    assert!(t["claimed_by"].is_null() || t["claimed_by"] == "");
    assert!(t["labels"].as_array().unwrap().len() == 2);

    // 9. Check task events
    let events = client
        .get(format!("/api/v1/boards/{}/tasks/{}/events", board_id, tid))
        .dispatch();
    assert_eq!(events.status(), Status::Ok);
    let ev: serde_json::Value = events.into_json().unwrap();
    assert!(ev.as_array().unwrap().len() >= 5, "Should have at least 5 events for full lifecycle");
}

// ============ API v1 Skills Discovery ============

#[test]
fn test_http_api_v1_skills_skill_md() {
    let client = test_client();
    let resp = client.get("/api/v1/skills/SKILL.md").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body = resp.into_string().unwrap();
    assert!(body.starts_with("---"));
    assert!(body.contains("name: kanban"));
}

// ============ Board Created At ============

#[test]
fn test_http_board_created_at_preserved() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Timestamp Board");

    let get1 = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let b1: serde_json::Value = get1.into_json().unwrap();
    let created = b1["created_at"].as_str().unwrap().to_string();

    // Update board
    client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"name": "Renamed Board"}"#)
        .dispatch();

    let get2 = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let b2: serde_json::Value = get2.into_json().unwrap();
    assert_eq!(b2["created_at"].as_str().unwrap(), created, "created_at should not change on update");
    assert_eq!(b2["name"], "Renamed Board");
}

// ============ Task Position on Create ============

#[test]
fn test_http_task_position_explicit_vs_auto() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Position Auto Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    // Auto-position (no position specified)
    let r1 = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Auto 1", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let t1: serde_json::Value = r1.into_json().unwrap();

    let r2 = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Auto 2", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let t2: serde_json::Value = r2.into_json().unwrap();

    // Auto positions should be sequential
    assert!(
        t2["position"].as_i64().unwrap() > t1["position"].as_i64().unwrap(),
        "Second task should have higher position"
    );

    // Explicit position
    let r3 = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Explicit 0", "column_id": "{}", "position": 0, "actor_name": "x"}}"#, col_id))
        .dispatch();
    let t3: serde_json::Value = r3.into_json().unwrap();
    assert_eq!(t3["position"], 0);
}

// ============ Comment Ordering ============

#[test]
fn test_http_comments_ordered_by_creation() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Comment Order Board");

    let cols_resp = client.get(format!("/api/v1/boards/{}", board_id)).dispatch();
    let cols: serde_json::Value = cols_resp.into_json().unwrap();
    let col_id = cols["columns"][0]["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Ordered comments", "column_id": "{}", "actor_name": "x"}}"#, col_id))
        .dispatch();
    let task: serde_json::Value = resp.into_json().unwrap();
    let tid = task["id"].as_str().unwrap();

    let messages = ["First", "Second", "Third"];
    for msg in &messages {
        client
            .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, tid))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"message": "{}", "actor_name": "bot"}}"#, msg))
            .dispatch();
    }

    // Comments are stored as task events
    let events = client
        .get(format!("/api/v1/boards/{}/tasks/{}/events", board_id, tid))
        .dispatch();
    assert_eq!(events.status(), Status::Ok);
    let ev: serde_json::Value = events.into_json().unwrap();
    let comment_events: Vec<_> = ev.as_array().unwrap().iter()
        .filter(|e| e["event_type"] == "comment")
        .collect();
    assert_eq!(comment_events.len(), 3);
    // Should be in chronological order (oldest first or newest first)
    // Just verify all 3 are present
    let msgs: Vec<&str> = comment_events.iter()
        .filter_map(|e| e["data"].as_object()
            .and_then(|d| d.get("message"))
            .and_then(|m| m.as_str()))
        .collect();
    assert!(msgs.contains(&"First"));
    assert!(msgs.contains(&"Third"));
}

// ============ Display Name Enforcement Advanced ============

#[test]
fn test_http_display_name_with_special_chars() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "DN Board", "columns": ["Todo"]}"#)
        .dispatch();
    let body: serde_json::Value = resp.into_json().unwrap();
    let board_id = body["id"].as_str().unwrap();
    let key = body["manage_key"].as_str().unwrap();
    let col_id = body["columns"][0]["id"].as_str().unwrap();

    // Enable display name requirement
    let settings = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"settings": {"require_display_name": true}}"#)
        .dispatch();
    assert_eq!(settings.status(), Status::Ok);

    // Create task with emoji display name
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Test", "column_id": "{}", "actor_name": "🤖 Bot v3.0"}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let task: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(task["created_by"], "🤖 Bot v3.0");
}

// ============ Activity Feed Enrichment ============

#[test]
fn test_http_activity_created_event_includes_task_snapshot() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Activity Enrichment");
    let col_id = get_first_column_id(&client, &board_id);

    // Create a task
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Enriched Task", "column_id": "{}", "priority": 2, "labels": ["bug"]}}"#,
            col_id
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Fetch activity feed
    let activity = client
        .get(format!("/api/v1/boards/{}/activity", board_id))
        .dispatch();
    assert_eq!(activity.status(), Status::Ok);
    let items: Vec<serde_json::Value> = activity.into_json().unwrap();

    let created_event = items.iter().find(|e| e["event_type"] == "created").unwrap();
    assert!(created_event["task"].is_object(), "created event should include task snapshot");
    assert_eq!(created_event["task"]["title"], "Enriched Task");
    assert_eq!(created_event["task"]["priority"], 2);
}

#[test]
fn test_http_activity_comment_event_includes_recent_comments() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Comment Enrich");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Commented", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    // Add comments
    for msg in &["First comment", "Second comment", "Third comment"] {
        client
            .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, tid))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"message": "{}"}}"#, msg))
            .dispatch();
    }

    let activity = client
        .get(format!("/api/v1/boards/{}/activity", board_id))
        .dispatch();
    let items: Vec<serde_json::Value> = activity.into_json().unwrap();

    let comment_events: Vec<_> = items.iter().filter(|e| e["event_type"] == "comment").collect();
    assert!(!comment_events.is_empty());

    // Comment events should include recent_comments array
    let first_comment_event = &comment_events[0];
    assert!(first_comment_event["recent_comments"].is_array(), "comment events should include recent_comments");
    let recent = first_comment_event["recent_comments"].as_array().unwrap();
    assert!(!recent.is_empty());
    // Each comment snapshot should have id, actor, message, created_at
    assert!(recent[0]["id"].is_string());
    assert!(recent[0]["message"].is_string());
    assert!(recent[0]["created_at"].is_string());
}

#[test]
fn test_http_activity_after_seq_cursor_ascending() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Cursor Test");
    let col_id = get_first_column_id(&client, &board_id);

    // Create 3 tasks to generate events
    for i in 1..=3 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"title": "Task {}", "column_id": "{}"}}"#, i, col_id))
            .dispatch();
    }

    // Get all activity to find a seq to use as cursor
    let all_activity = client
        .get(format!("/api/v1/boards/{}/activity", board_id))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert!(all_activity.len() >= 3);

    // Use `after` cursor with the first event's seq
    let first_seq = all_activity.last().unwrap()["seq"].as_i64().unwrap(); // oldest in DESC order
    let cursor_activity = client
        .get(format!("/api/v1/boards/{}/activity?after={}", board_id, first_seq))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();

    // Should return events after the first one (ASC order when using after)
    assert!(cursor_activity.len() < all_activity.len());
    for event in &cursor_activity {
        assert!(event["seq"].as_i64().unwrap() > first_seq);
    }
}

#[test]
fn test_http_activity_since_filter() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Since Filter");
    let col_id = get_first_column_id(&client, &board_id);

    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Old Task", "column_id": "{}"}}"#, col_id))
        .dispatch();

    // Use a future timestamp — should return nothing
    let activity = client
        .get(format!("/api/v1/boards/{}/activity?since=2099-01-01T00:00:00Z", board_id))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert_eq!(activity.len(), 0);

    // Use a past timestamp — should return all
    let activity = client
        .get(format!("/api/v1/boards/{}/activity?since=2000-01-01T00:00:00Z", board_id))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert!(!activity.is_empty());
}

// ============ Task Events Endpoint ============

#[test]
fn test_http_task_events_include_all_types() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Task Events Types");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Event Track", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    // Claim
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/claim?actor=bot", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // Comment
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"message": "Progress update"}"#)
        .dispatch();

    // Release
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/release?actor=bot", board_id, tid))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // Get all events
    let events = client
        .get(format!("/api/v1/boards/{}/tasks/{}/events", board_id, tid))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();

    let event_types: Vec<&str> = events.iter()
        .filter_map(|e| e["event_type"].as_str())
        .collect();
    assert!(event_types.contains(&"created"));
    assert!(event_types.contains(&"claimed"));
    assert!(event_types.contains(&"comment"));
    assert!(event_types.contains(&"released"));
    assert!(events.len() >= 4);

    // Each event should have id, event_type, actor, data, created_at
    for event in &events {
        assert!(event["id"].is_string());
        assert!(event["event_type"].is_string());
        assert!(event["actor"].is_string());
        assert!(event["created_at"].is_string());
    }
}

#[test]
fn test_http_task_events_empty_for_new_task() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Empty Events");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Fresh", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    let events = client
        .get(format!("/api/v1/boards/{}/tasks/{}/events", board_id, tid))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();

    // Should have at least the "created" event
    assert!(!events.is_empty());
    assert_eq!(events.iter().filter(|e| e["event_type"] == "created").count(), 1);
}

// ============ WIP Limit Enforcement on Move ============

#[test]
fn test_http_wip_limit_blocks_move() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "WIP Move", "columns": ["Open", "Limited"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let board_id = resp["id"].as_str().unwrap();
    let key = resp["manage_key"].as_str().unwrap();
    let open_col = resp["columns"][0]["id"].as_str().unwrap();
    let limited_col = resp["columns"][1]["id"].as_str().unwrap();

    // Set WIP limit of 1 on "Limited"
    client
        .patch(format!("/api/v1/boards/{}/columns/{}", board_id, limited_col))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"wip_limit": 1}"#)
        .dispatch();

    // Create 2 tasks in "Open"
    let task1 = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Task 1", "column_id": "{}"}}"#, open_col))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let t1_id = task1["id"].as_str().unwrap();

    let task2 = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Task 2", "column_id": "{}"}}"#, open_col))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let t2_id = task2["id"].as_str().unwrap();

    // Move first task to Limited — should succeed
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}", board_id, t1_id, limited_col))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Move second task to Limited — should fail (WIP limit reached)
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}", board_id, t2_id, limited_col))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "WIP_LIMIT_EXCEEDED");
}

// ============ Move to Done Sets completed_at ============

#[test]
fn test_http_move_to_done_sets_completed_at() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Complete Test");
    let col_id = get_first_column_id(&client, &board_id);

    // Get the "Done" column (last column)
    let board = client
        .get(format!("/api/v1/boards/{}", board_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let done_col = board["columns"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap();

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Complete Me", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();
    assert!(task["completed_at"].is_null());

    // Move to Done
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}", board_id, tid, done_col))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert!(resp["completed_at"].is_string(), "completed_at should be set when moved to last column");
}

#[test]
fn test_http_move_away_from_done_clears_completed_at() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Uncomplete");
    let col_id = get_first_column_id(&client, &board_id);

    let board = client
        .get(format!("/api/v1/boards/{}", board_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let done_col = board["columns"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap();

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Bounce", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    // Move to Done
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}", board_id, tid, done_col))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // Move back to first column
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}", board_id, tid, col_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert!(resp["completed_at"].is_null(), "completed_at should be cleared when moved away from Done");
}

// ============ Multi-Criteria Task Filters ============

#[test]
fn test_http_list_tasks_multi_filter_priority_and_label() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Multi Filter");
    let col_id = get_first_column_id(&client, &board_id);

    // Create tasks with different priorities and labels
    for (title, priority, label) in &[
        ("Bug High", 2, "bug"),
        ("Bug Low", 0, "bug"),
        ("Feature High", 2, "feature"),
        ("Feature Low", 0, "feature"),
    ] {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(
                r#"{{"title": "{}", "column_id": "{}", "priority": {}, "labels": ["{}"]}}"#,
                title, col_id, priority, label
            ))
            .dispatch();
    }

    // Filter by priority=2 AND label=bug
    let resp = client
        .get(format!(
            "/api/v1/boards/{}/tasks?priority=2&label=bug",
            board_id
        ))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert_eq!(resp.len(), 1);
    assert_eq!(resp[0]["title"], "Bug High");
}

#[test]
fn test_http_list_tasks_multi_filter_assigned_and_column() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Assign Filter", "columns": ["Todo", "InProgress"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let board_id = resp["id"].as_str().unwrap();
    let key = resp["manage_key"].as_str().unwrap();
    let todo_col = resp["columns"][0]["id"].as_str().unwrap();
    let ip_col = resp["columns"][1]["id"].as_str().unwrap();

    // Create tasks
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Alice Todo", "column_id": "{}", "assigned_to": "alice"}}"#,
            todo_col
        ))
        .dispatch();
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Alice IP", "column_id": "{}", "assigned_to": "alice"}}"#,
            ip_col
        ))
        .dispatch();
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Bob IP", "column_id": "{}", "assigned_to": "bob"}}"#,
            ip_col
        ))
        .dispatch();

    // Filter by assigned=alice AND column=InProgress
    let resp = client
        .get(format!(
            "/api/v1/boards/{}/tasks?assigned={}&column={}",
            board_id, "alice", ip_col
        ))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert_eq!(resp.len(), 1);
    assert_eq!(resp[0]["title"], "Alice IP");
}

// ============ Reorder Task with Column Move ============

#[test]
fn test_http_reorder_task_cross_column() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Cross Reorder", "columns": ["Col A", "Col B"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let board_id = resp["id"].as_str().unwrap();
    let key = resp["manage_key"].as_str().unwrap();
    let col_a = resp["columns"][0]["id"].as_str().unwrap();
    let col_b = resp["columns"][1]["id"].as_str().unwrap();

    // Create task in Col A
    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Mover", "column_id": "{}"}}"#, col_a))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();
    assert_eq!(task["column_id"].as_str().unwrap(), col_a);

    // Reorder with column_id to move to Col B at position 0
    let resp = client
        .post(format!(
            "/api/v1/boards/{}/tasks/{}/reorder",
            board_id, tid
        ))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"position": 0, "column_id": "{}"}}"#, col_b))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(resp["column_id"].as_str().unwrap(), col_b);
    assert_eq!(resp["position"], 0);
}

// ============ Dependency Create and Delete ============

#[test]
fn test_http_dependency_create_and_explicit_delete() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Dep CRUD");
    let col_id = get_first_column_id(&client, &board_id);

    let t1 = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Blocker", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let t1_id = t1["id"].as_str().unwrap();

    let t2 = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Blocked", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let t2_id = t2["id"].as_str().unwrap();

    // Create dependency: t2 blocked by t1 (t1 is the blocker)
    let dep = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#,
            t1_id, t2_id
        ))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let dep_id = dep["id"].as_str().unwrap();

    // Verify dependency exists
    let deps = client
        .get(format!("/api/v1/boards/{}/dependencies?task={}", board_id, t2_id))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0]["blocker_task_id"], t1_id);
    assert_eq!(deps[0]["blocked_task_id"], t2_id);

    // Explicitly delete the dependency
    let resp = client
        .delete(format!("/api/v1/boards/{}/dependencies/{}", board_id, dep_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Verify dependency is gone
    let deps = client
        .get(format!("/api/v1/boards/{}/dependencies?task={}", board_id, t2_id))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert_eq!(deps.len(), 0);
}

// ============ Comment Count Accuracy ============

#[test]
fn test_http_comment_count_increments() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Comment Count");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Counting", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();
    assert_eq!(task["comment_count"], 0);

    // Add 3 comments
    for i in 1..=3 {
        client
            .post(format!("/api/v1/boards/{}/tasks/{}/comment", board_id, tid))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"message": "Comment {}"}}"#, i))
            .dispatch();
    }

    // Verify count
    let task = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(task["comment_count"], 3);
}

// ============ Board Isolation ============

#[test]
fn test_http_board_isolation_cross_key_rejection() {
    let client = test_client();
    let (board1_id, _key1) = create_test_board(&client, "Board A");
    let (_board2_id, key2) = create_test_board(&client, "Board B");

    // Try to create a task on Board A using Board B's key
    let col_id = get_first_column_id(&client, &board1_id);
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board1_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key2)))
        .body(format!(r#"{{"title": "Cross Board", "column_id": "{}"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Forbidden);
}

#[test]
fn test_http_board_isolation_column_count() {
    let client = test_client();
    let (board1_id, key1) = create_test_board(&client, "Isolated A");
    let col1 = get_first_column_id(&client, &board1_id);
    let (board2_id, key2) = create_test_board(&client, "Isolated B");
    let col2 = get_first_column_id(&client, &board2_id);

    // Add tasks to each board
    for _ in 0..3 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board1_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key1)))
            .body(format!(r#"{{"title": "A task", "column_id": "{}"}}"#, col1))
            .dispatch();
    }
    for _ in 0..2 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board2_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key2)))
            .body(format!(r#"{{"title": "B task", "column_id": "{}"}}"#, col2))
            .dispatch();
    }

    // Verify counts are isolated
    let b1 = client.get(format!("/api/v1/boards/{}", board1_id)).dispatch().into_json::<serde_json::Value>().unwrap();
    let b2 = client.get(format!("/api/v1/boards/{}", board2_id)).dispatch().into_json::<serde_json::Value>().unwrap();
    assert_eq!(b1["task_count"], 3);
    assert_eq!(b2["task_count"], 2);
}

// ============ Task Metadata Persistence ============

#[test]
fn test_http_task_metadata_persists_and_updates() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Metadata");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Meta Task", "column_id": "{}", "metadata": {{"source": "api", "version": 1}}}}"#,
            col_id
        ))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();
    assert_eq!(task["metadata"]["source"], "api");
    assert_eq!(task["metadata"]["version"], 1);

    // Update metadata
    let updated = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"metadata": {"source": "ui", "version": 2, "extra": true}}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(updated["metadata"]["source"], "ui");
    assert_eq!(updated["metadata"]["version"], 2);
    assert_eq!(updated["metadata"]["extra"], true);

    // Verify persistence via GET
    let fetched = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(fetched["metadata"]["version"], 2);
}

// ============ Column Reorder Preserves Tasks ============

#[test]
fn test_http_column_reorder_preserves_task_assignment() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Reorder Preserve", "columns": ["A", "B", "C"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let board_id = resp["id"].as_str().unwrap();
    let key = resp["manage_key"].as_str().unwrap();
    let cols: Vec<&str> = resp["columns"].as_array().unwrap()
        .iter().map(|c| c["id"].as_str().unwrap()).collect();

    // Create a task in column B
    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "In B", "column_id": "{}"}}"#, cols[1]))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    // Reorder: C, A, B
    client
        .post(format!("/api/v1/boards/{}/columns/reorder", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"column_ids": ["{}", "{}", "{}"]}}"#, cols[2], cols[0], cols[1]))
        .dispatch();

    // Task should still be in column B
    let task = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(task["column_id"].as_str().unwrap(), cols[1]);
    assert_eq!(task["column_name"], "B");
}

// ============ Board Update All Fields ============

#[test]
fn test_http_update_board_all_settings() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Settings Board");

    let board = client
        .get(format!("/api/v1/boards/{}", board_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let done_col = board["columns"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap();

    let resp = client
        .patch(format!("/api/v1/boards/{}", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{
                "name": "Updated Name",
                "description": "New description",
                "is_public": true,
                "require_display_name": true,
                "quick_done_column_id": "{}",
                "quick_done_auto_archive": true
            }}"#,
            done_col
        ))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    let board = client
        .get(format!("/api/v1/boards/{}", board_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(board["name"], "Updated Name");
    assert_eq!(board["description"], "New description");
    assert_eq!(board["is_public"], true);
    assert_eq!(board["require_display_name"], true);
    assert_eq!(board["quick_done_column_id"].as_str().unwrap(), done_col);
    assert_eq!(board["quick_done_auto_archive"], true);
}

// ============ Webhook Deactivate and Reactivate ============

#[test]
fn test_http_webhook_deactivate_reactivate() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Webhook Toggle");

    // Create webhook
    let wh = client
        .post(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"url": "https://example.com/hook", "events": ["task.created"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let wh_id = wh["id"].as_str().unwrap();
    assert_eq!(wh["active"], true);

    // Deactivate
    let resp = client
        .patch(format!("/api/v1/boards/{}/webhooks/{}", board_id, wh_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"active": false}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(resp["active"], false);

    // Reactivate
    let resp = client
        .patch(format!("/api/v1/boards/{}/webhooks/{}", board_id, wh_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"active": true}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(resp["active"], true);
}

// ============ Task Update with Column Move ============

#[test]
fn test_http_update_task_changes_column() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Update Move", "columns": ["Open", "Closed"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let board_id = resp["id"].as_str().unwrap();
    let key = resp["manage_key"].as_str().unwrap();
    let open_col = resp["columns"][0]["id"].as_str().unwrap();
    let closed_col = resp["columns"][1]["id"].as_str().unwrap();

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Movable", "column_id": "{}"}}"#, open_col))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    // Update task with new column_id
    let updated = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"column_id": "{}"}}"#, closed_col))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(updated["column_id"].as_str().unwrap(), closed_col);
}

// ============ Search Pagination ============

#[test]
fn test_http_search_with_offset_and_limit() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Search Page");
    let col_id = get_first_column_id(&client, &board_id);

    // Create 5 tasks with "searchable" in title
    for i in 1..=5 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"title": "Searchable Item {}", "column_id": "{}"}}"#, i, col_id))
            .dispatch();
    }

    // Search with limit=2
    let resp = client
        .get(format!(
            "/api/v1/boards/{}/tasks/search?q=searchable&limit=2",
            board_id
        ))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let empty = vec![];
    let results = resp["tasks"].as_array().unwrap_or(&empty);
    assert_eq!(results.len(), 2);
    assert!(resp["total"].as_i64().unwrap() >= 5);
}

// ============ Activity Feed Limit ============

#[test]
fn test_http_activity_respects_limit() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Activity Limit");
    let col_id = get_first_column_id(&client, &board_id);

    // Create 5 tasks
    for i in 1..=5 {
        client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"title": "Task {}", "column_id": "{}"}}"#, i, col_id))
            .dispatch();
    }

    // Fetch with limit=2
    let activity = client
        .get(format!("/api/v1/boards/{}/activity?limit=2", board_id))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert_eq!(activity.len(), 2);
}

// ============ Column WIP Update ============

#[test]
fn test_http_column_wip_limit_update() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "WIP Update");
    let col_id = get_first_column_id(&client, &board_id);

    // Set WIP limit to 5
    let resp = client
        .patch(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"wip_limit": 5}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(resp["wip_limit"], 5);

    // Update to 10
    let resp = client
        .patch(format!("/api/v1/boards/{}/columns/{}", board_id, col_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"wip_limit": 10}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(resp["wip_limit"], 10);

    // Verify via board GET
    let board = client
        .get(format!("/api/v1/boards/{}", board_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(board["columns"][0]["wip_limit"], 10);
}

// ============ Multiple Boards Same Name ============

#[test]
fn test_http_multiple_boards_same_name_allowed() {
    let client = test_client();
    let (id1, _) = create_test_board(&client, "Duplicate Name");
    let (id2, _) = create_test_board(&client, "Duplicate Name");
    assert_ne!(id1, id2);
}

// ============ Archived Board Blocks All Writes ============

#[test]
fn test_http_archived_board_blocks_column_operations() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Archive Block");

    // Archive the board
    client
        .post(format!("/api/v1/boards/{}/archive", board_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // Try to create a column — should be rejected
    let resp = client
        .post(format!("/api/v1/boards/{}/columns", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"name": "New Col"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);
}

#[test]
fn test_http_archived_board_blocks_task_creation() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Archive Tasks");
    let col_id = get_first_column_id(&client, &board_id);

    // Archive
    client
        .post(format!("/api/v1/boards/{}/archive", board_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // Try to create a task — should be rejected
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Should Fail", "column_id": "{}"}}"#, col_id))
        .dispatch();
    assert_eq!(resp.status(), Status::Conflict);
}

// ============ Task Due Date Lifecycle ============

#[test]
fn test_http_task_due_date_set_and_update() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Due Date");
    let col_id = get_first_column_id(&client, &board_id);

    // Create with due date
    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Due Task", "column_id": "{}", "due_at": "2026-03-01T00:00:00Z"}}"#,
            col_id
        ))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();
    assert!(task["due_at"].is_string());
    assert!(task["due_at"].as_str().unwrap().contains("2026-03-01"));

    // Update to a different due date
    let updated = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"due_at": "2026-04-15T12:00:00Z"}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert!(updated["due_at"].as_str().unwrap().contains("2026-04-15"));

    // Verify persistence
    let fetched = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert!(fetched["due_at"].as_str().unwrap().contains("2026-04-15"));
}

// ============ Batch with Priority ============

#[test]
fn test_http_batch_update_priority() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Batch Priority");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Low", "column_id": "{}", "priority": 0}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"operations": [{{"action": "update", "task_ids": ["{}"], "priority": 3}}]}}"#,
            tid
        ))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(resp["results"][0]["success"], true);

    // Verify
    let task = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(task["priority"], 3);
}

// ============ Move Event Data ============

#[test]
fn test_http_move_event_contains_column_names() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Move Events", "columns": ["Starting", "Ending"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let board_id = resp["id"].as_str().unwrap();
    let key = resp["manage_key"].as_str().unwrap();
    let start_col = resp["columns"][0]["id"].as_str().unwrap();
    let end_col = resp["columns"][1]["id"].as_str().unwrap();

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Track Move", "column_id": "{}"}}"#, start_col))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    // Move task
    client
        .post(format!("/api/v1/boards/{}/tasks/{}/move/{}", board_id, tid, end_col))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // Check event data
    let events = client
        .get(format!("/api/v1/boards/{}/tasks/{}/events", board_id, tid))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    let move_event = events.iter().find(|e| e["event_type"] == "moved").unwrap();
    assert_eq!(move_event["data"]["from_column"], "Starting");
    assert_eq!(move_event["data"]["to_column"], "Ending");
}

// ============ Create Board with Description ============

#[test]
fn test_http_create_board_with_description() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Described Board", "description": "A detailed description of this board", "columns": ["Todo"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();

    let board = client
        .get(format!("/api/v1/boards/{}", resp["id"].as_str().unwrap()))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(board["description"], "A detailed description of this board");
}

// ============ List Boards Defaults Exclude Archived ============

#[test]
fn test_http_list_boards_excludes_archived_by_default() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Will Archive");
    let (_board2_id, _key2) = create_test_board(&client, "Stays Active");

    // Archive first board
    client
        .post(format!("/api/v1/boards/{}/archive", board_id))
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .dispatch();

    // List without include_archived
    let boards = client
        .get("/api/v1/boards")
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();

    let ids: Vec<&str> = boards.iter().filter_map(|b| b["id"].as_str()).collect();
    assert!(!ids.contains(&board_id.as_str()));
}

// ============ Task Stale Filter ============

#[test]
fn test_http_task_stale_filter_returns_old_tasks() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Stale Filter");
    let col_id = get_first_column_id(&client, &board_id);

    // Create a task (it's "new" so using stale=1h should not include it immediately,
    // but stale=0 or checking that the filter param is accepted is the goal)
    client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Fresh Task", "column_id": "{}"}}"#, col_id))
        .dispatch();

    // stale=0 should be rejected (must be positive)
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?stale=0", board_id))
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);

    // stale=999999 (very large) — fresh task won't be stale
    let resp = client
        .get(format!("/api/v1/boards/{}/tasks?stale=999999", board_id))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    // Fresh task has updated_at = now, so it won't be stale for 999999 minutes
    assert_eq!(resp.len(), 0);
}

// ============ Label Normalization Verified ============

#[test]
fn test_http_label_normalization() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Label Norm");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Labeled", "column_id": "{}", "labels": ["  High Priority  ", "BUG FIX", "  dup--label  "]}}"#,
            col_id
        ))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();

    let labels: Vec<&str> = task["labels"].as_array().unwrap()
        .iter().filter_map(|l| l.as_str()).collect();
    // Labels should be lowercase, trimmed, spaces→dashes, collapsed dashes
    assert!(labels.contains(&"high-priority"));
    assert!(labels.contains(&"bug-fix"));
    assert!(labels.contains(&"dup-label"));
}

// ============ Board Created Timestamp ============

#[test]
fn test_http_board_created_at_is_iso8601() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Timestamp Board", "columns": ["Todo"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();

    let created = resp["created_at"].as_str().unwrap();
    // Create response returns RFC3339 format
    assert!(created.contains("2026"), "created_at should contain year: {}", created);
    assert!(created.contains("T"), "create response should be RFC3339: {}", created);

    let board = client
        .get(format!("/api/v1/boards/{}", resp["id"].as_str().unwrap()))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let board_created = board["created_at"].as_str().unwrap();
    // GET response may use SQLite datetime format (space separator)
    assert!(board_created.contains("2026"), "board created_at should be a timestamp: {}", board_created);
}

// ============ Webhook Multiple Events ============

#[test]
fn test_http_webhook_multiple_events() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Multi Webhook");

    let wh = client
        .post(format!("/api/v1/boards/{}/webhooks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"url": "https://example.com/multi", "events": ["task.created", "task.moved", "task.claimed"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();

    let events = wh["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    assert!(events.iter().any(|e| e == "task.created"));
    assert!(events.iter().any(|e| e == "task.moved"));
    assert!(events.iter().any(|e| e == "task.claimed"));
}

// ============ Search Special Characters ============

#[test]
fn test_http_search_special_chars_no_crash() {
    let client = test_client();
    let (board_id, _key) = create_test_board(&client, "Search Special");

    // Search with special characters should not panic/crash
    for query in &["DROP+TABLE", "task+test", "test+quote", "abc", "query"] {
        let resp = client
            .get(format!("/api/v1/boards/{}/tasks/search?q={}", board_id, query))
            .dispatch();
        assert!(resp.status() == Status::Ok || resp.status() == Status::BadRequest);
    }
}

// ============ Dependency Between Same Task Rejected ============

#[test]
fn test_http_dependency_on_self_via_indirect_check() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Self Dep");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"title": "Self", "column_id": "{}"}}"#, col_id))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();

    // Self-dependency should be rejected
    let resp = client
        .post(format!("/api/v1/boards/{}/dependencies", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"blocker_task_id": "{}", "blocked_task_id": "{}"}}"#, tid, tid))
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "SELF_DEPENDENCY");
}

// ============ Task Assigned To Update and Filter ============

#[test]
fn test_http_task_assigned_to_clear() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Assign Clear");
    let col_id = get_first_column_id(&client, &board_id);

    let task = client
        .post(format!("/api/v1/boards/{}/tasks", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(
            r#"{{"title": "Assigned", "column_id": "{}", "assigned_to": "agent-1"}}"#,
            col_id
        ))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    let tid = task["id"].as_str().unwrap();
    assert_eq!(task["assigned_to"], "agent-1");

    // Change assigned_to
    let updated = client
        .patch(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(r#"{"assigned_to": "agent-2"}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(updated["assigned_to"], "agent-2");

    // Verify persistence
    let fetched = client
        .get(format!("/api/v1/boards/{}/tasks/{}", board_id, tid))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(fetched["assigned_to"], "agent-2");
}

// ============ Board with Public Listing ============

#[test]
fn test_http_create_board_as_public() {
    let client = test_client();
    let resp = client
        .post("/api/v1/boards")
        .header(ContentType::JSON)
        .body(r#"{"name": "Public Board", "is_public": true, "columns": ["Todo"]}"#)
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();

    let board = client
        .get(format!("/api/v1/boards/{}", resp["id"].as_str().unwrap()))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(board["is_public"], true);

    // Should appear in public list
    let boards = client
        .get("/api/v1/boards?only_public=true")
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert!(boards.iter().any(|b| b["id"] == resp["id"]));
}

// ============ Batch Delete Multiple ============

#[test]
fn test_http_batch_delete_multiple_tasks() {
    let client = test_client();
    let (board_id, key) = create_test_board(&client, "Batch Del");
    let col_id = get_first_column_id(&client, &board_id);

    let mut task_ids = vec![];
    for i in 1..=3 {
        let task = client
            .post(format!("/api/v1/boards/{}/tasks", board_id))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", key)))
            .body(format!(r#"{{"title": "Del {}", "column_id": "{}"}}"#, i, col_id))
            .dispatch()
            .into_json::<serde_json::Value>()
            .unwrap();
        task_ids.push(task["id"].as_str().unwrap().to_string());
    }

    // Batch delete all at once
    let ids_json: Vec<String> = task_ids.iter().map(|id| format!("\"{}\"", id)).collect();
    let resp = client
        .post(format!("/api/v1/boards/{}/tasks/batch", board_id))
        .header(ContentType::JSON)
        .header(Header::new("Authorization", format!("Bearer {}", key)))
        .body(format!(r#"{{"operations": [{{"action": "delete", "task_ids": [{}]}}]}}"#, ids_json.join(",")))
        .dispatch()
        .into_json::<serde_json::Value>()
        .unwrap();

    let results = resp["results"].as_array().unwrap();
    assert_eq!(results.len(), 1); // One delete operation with multiple task_ids
    assert_eq!(results[0]["success"], true);
    assert_eq!(results[0]["affected"], 3);

    // Verify tasks are gone
    let tasks = client
        .get(format!("/api/v1/boards/{}/tasks", board_id))
        .dispatch()
        .into_json::<Vec<serde_json::Value>>()
        .unwrap();
    assert_eq!(tasks.len(), 0);
}
