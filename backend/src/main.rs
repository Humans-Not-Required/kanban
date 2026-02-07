#[macro_use]
extern crate rocket;

mod access;
mod auth;
mod db;
mod events;
mod models;
mod rate_limit;
mod routes;
mod webhooks;

use std::path::PathBuf;
use std::time::Duration;

use events::EventBus;
use rate_limit::{RateLimitHeaders, RateLimiter};
use rocket::fs::{FileServer, Options};
use rocket_cors::{AllowedOrigins, CorsOptions};

#[launch]
fn rocket() -> _ {
    let _ = dotenvy::dotenv();

    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::all())
        .to_cors()
        .expect("CORS configuration failed");

    // Rate limit window: configurable via RATE_LIMIT_WINDOW_SECS (default: 60s)
    let window_secs: u64 = std::env::var("RATE_LIMIT_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    // Frontend static files directory (default: ../frontend/dist relative to CWD)
    let static_dir: PathBuf = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));

    // Initialize main database
    let db = db::init_db().expect("Failed to initialize database");

    // Initialize a separate DB connection for async webhook delivery
    let webhook_db = db::init_webhook_db().expect("Failed to initialize webhook database");

    let mut build = rocket::build()
        .attach(cors)
        .attach(RateLimitHeaders)
        .manage(db)
        .manage(RateLimiter::new(Duration::from_secs(window_secs)))
        .manage(EventBus::with_webhooks(webhook_db))
        .mount(
            "/api/v1",
            routes![
                routes::health,
                routes::openapi,
                // Boards
                routes::create_board,
                routes::list_boards,
                routes::get_board,
                routes::archive_board,
                routes::unarchive_board,
                // Columns
                routes::create_column,
                // Tasks
                routes::create_task,
                routes::search_tasks,
                routes::list_tasks,
                routes::get_task,
                routes::update_task,
                routes::delete_task,
                // Batch operations
                routes::batch_tasks,
                // Agent-first: claim/release/move/reorder
                routes::claim_task,
                routes::release_task,
                routes::move_task,
                routes::reorder_task,
                // Task events & comments
                routes::get_task_events,
                routes::comment_on_task,
                // SSE event stream
                routes::board_event_stream,
                // Board collaborators
                routes::list_collaborators,
                routes::add_collaborator,
                routes::remove_collaborator,
                // Task dependencies
                routes::create_dependency,
                routes::list_dependencies,
                routes::delete_dependency,
                // Webhooks
                routes::create_webhook,
                routes::list_webhooks,
                routes::update_webhook,
                routes::delete_webhook,
                // API keys
                routes::list_keys,
                routes::create_key,
                routes::delete_key,
            ],
        );

    // Serve frontend static files if the directory exists
    if static_dir.is_dir() {
        println!("📦 Serving frontend from: {}", static_dir.display());
        build = build
            .mount("/", FileServer::new(&static_dir, Options::Index))
            .mount("/", routes![routes::spa_fallback]);
    } else {
        println!(
            "⚠️  Frontend directory not found: {} (API-only mode)",
            static_dir.display()
        );
    }

    build
}
