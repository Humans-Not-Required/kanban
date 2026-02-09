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
use rate_limit::RateLimiter;
use rocket::fs::{FileServer, Options};
use rocket_cors::{AllowedOrigins, CorsOptions};

use kanban::catchers::*;

#[launch]
fn rocket() -> _ {
    let _ = dotenvy::dotenv();

    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::all())
        .to_cors()
        .expect("CORS configuration failed");

    // Frontend static files directory (default: ../frontend/dist relative to CWD)
    let static_dir: PathBuf = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));

    // Initialize main database
    let db = db::init_db().expect("Failed to initialize database");

    // Initialize a separate DB connection for async webhook delivery
    let webhook_db = db::init_webhook_db().expect("Failed to initialize webhook database");

    // Board creation rate limiter: 10 boards per hour per IP
    let board_rate_limit = std::env::var("BOARD_RATE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);
    let board_rate_limiter = RateLimiter::new(Duration::from_secs(3600), board_rate_limit);

    let mut build = rocket::build()
        .attach(cors)
        .register("/", catchers![unauthorized, not_found, unprocessable, too_many_requests, internal_error])
        .manage(db)
        .manage(board_rate_limiter)
        .manage(EventBus::with_webhooks(webhook_db))
        .mount(
            "/api/v1",
            routes![
                routes::health,
                routes::openapi,
                routes::llms_txt,
                // Boards (create = no auth, list = public only)
                routes::create_board,
                routes::list_boards,
                routes::get_board,
                routes::update_board,
                routes::archive_board,
                routes::unarchive_board,
                // Columns (manage key required)
                routes::create_column,
                routes::update_column,
                routes::delete_column,
                routes::reorder_columns,
                // Tasks (read = public, write = manage key)
                routes::create_task,
                routes::search_tasks,
                routes::list_tasks,
                routes::get_task,
                routes::update_task,
                routes::delete_task,
                routes::archive_task,
                routes::unarchive_task,
                // Batch operations (manage key required)
                routes::batch_tasks,
                // Agent-first: claim/release/move/reorder (manage key required)
                routes::claim_task,
                routes::release_task,
                routes::move_task,
                routes::reorder_task,
                // Task events (read = public) & comments (manage key required)
                routes::get_task_events,
                routes::comment_on_task,
                // SSE event stream (public)
                routes::board_event_stream,
                // Task dependencies (read = public, write = manage key)
                routes::create_dependency,
                routes::list_dependencies,
                routes::delete_dependency,
                // Webhooks (manage key required)
                routes::create_webhook,
                routes::list_webhooks,
                routes::update_webhook,
                routes::delete_webhook,
            ],
        );

    // Mount llms.txt at root level for standard discovery
    build = build.mount("/", routes![routes::root_llms_txt]);

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
