#[macro_use]
extern crate rocket;

mod access;
mod auth;
mod db;
mod events;
mod models;
mod rate_limit;
mod routes;

use std::time::Duration;

use events::EventBus;
use rate_limit::{RateLimitHeaders, RateLimiter};
use rocket::fairing::AdHoc;
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

    rocket::build()
        .attach(cors)
        .attach(RateLimitHeaders)
        .attach(AdHoc::on_ignite("Database", |rocket| async {
            let db = db::init_db().expect("Failed to initialize database");
            rocket.manage(db)
        }))
        .manage(RateLimiter::new(Duration::from_secs(window_secs)))
        .manage(EventBus::new())
        .mount(
            "/api/v1",
            routes![
                routes::health,
                routes::openapi,
                // Boards
                routes::create_board,
                routes::list_boards,
                routes::get_board,
                // Columns
                routes::create_column,
                // Tasks
                routes::create_task,
                routes::search_tasks,
                routes::list_tasks,
                routes::get_task,
                routes::update_task,
                routes::delete_task,
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
                // API keys
                routes::list_keys,
                routes::create_key,
                routes::delete_key,
            ],
        )
}
