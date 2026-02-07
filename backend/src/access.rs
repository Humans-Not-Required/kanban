use crate::models::ApiError;
use rocket::http::Status;
use rocket::serde::json::Json;
use rusqlite::Connection;

/// Check if a board exists. Returns Err(404) if not.
pub fn require_board_exists(
    conn: &Connection,
    board_id: &str,
) -> Result<(), (Status, Json<ApiError>)> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if exists {
        Ok(())
    } else {
        Err((
            Status::NotFound,
            Json(ApiError {
                error: "Board not found".to_string(),
                code: "NOT_FOUND".to_string(),
                status: 404,
            }),
        ))
    }
}

/// Check if a board is archived. Returns error if it is.
pub fn require_not_archived(
    conn: &Connection,
    board_id: &str,
) -> Result<(), (Status, Json<ApiError>)> {
    let archived: bool = conn
        .query_row(
            "SELECT archived = 1 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if archived {
        Err((
            Status::Conflict,
            Json(ApiError {
                error: "Board is archived. Unarchive it before making changes.".to_string(),
                code: "BOARD_ARCHIVED".to_string(),
                status: 409,
            }),
        ))
    } else {
        Ok(())
    }
}

/// Verify that the given token hash matches the board's manage_key_hash.
/// Used by write routes to authorize modifications.
pub fn require_manage_key(
    conn: &Connection,
    board_id: &str,
    token_hash: &str,
) -> Result<(), (Status, Json<ApiError>)> {
    require_board_exists(conn, board_id)?;

    let stored_hash: String = conn
        .query_row(
            "SELECT manage_key_hash FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .map_err(|_| {
            (
                Status::NotFound,
                Json(ApiError {
                    error: "Board not found".to_string(),
                    code: "NOT_FOUND".to_string(),
                    status: 404,
                }),
            )
        })?;

    if stored_hash == token_hash {
        Ok(())
    } else {
        Err((
            Status::Forbidden,
            Json(ApiError {
                error: "Invalid management key for this board".to_string(),
                code: "INVALID_KEY".to_string(),
                status: 403,
            }),
        ))
    }
}
