use rocket::http::Status;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::tokio::select;
use rocket::tokio::time::Duration;
use rocket::{Shutdown, State};

use crate::access;
use crate::db::DbPool;
use crate::events::EventBus;
use crate::models::ApiError;

/// Public: anyone with the board UUID can subscribe to events.
#[get("/boards/<board_id>/events/stream")]
pub fn board_event_stream(
    board_id: &str,
    db: &State<DbPool>,
    bus: &State<EventBus>,
    mut shutdown: Shutdown,
) -> Result<EventStream![], (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    access::require_board_exists(&conn, board_id)?;
    drop(conn);

    let mut rx = bus.subscribe(board_id);

    Ok(EventStream! {
        loop {
            select! {
                msg = rx.recv() => match msg {
                    Ok(event) => {
                        yield Event::json(&event.data).event(event.event);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        yield Event::data("events_lost").event("warning".to_string());
                    }
                },
                _ = &mut shutdown => break,
            }
        }
    }
    .heartbeat(Duration::from_secs(15)))
}
