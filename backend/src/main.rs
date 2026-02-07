#[macro_use]
extern crate rocket;

mod auth;
mod db;
mod models;
mod routes;

use rocket::fairing::AdHoc;
use rocket_cors::{AllowedOrigins, CorsOptions};

#[launch]
fn rocket() -> _ {
    let _ = dotenvy::dotenv();

    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::all())
        .to_cors()
        .expect("CORS configuration failed");

    rocket::build()
        .attach(cors)
        .attach(AdHoc::on_ignite("Database", |rocket| async {
            let db = db::init_db().expect("Failed to initialize database");
            rocket.manage(db)
        }))
        .mount(
            "/api/v1",
            routes![
                routes::health,
                // Boards
                routes::create_board,
                routes::list_boards,
                routes::get_board,
                // Columns
                routes::create_column,
                // Tasks
                routes::create_task,
                routes::list_tasks,
                routes::get_task,
                routes::update_task,
                routes::delete_task,
                // Agent-first: claim/release/move
                routes::claim_task,
                routes::release_task,
                routes::move_task,
                // Task events & comments
                routes::get_task_events,
                routes::comment_on_task,
                // API keys
                routes::list_keys,
                routes::create_key,
                routes::delete_key,
            ],
        )
}
