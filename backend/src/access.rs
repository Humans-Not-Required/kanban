use crate::auth::AuthenticatedKey;
use crate::models::ApiError;
use rocket::http::Status;
use rocket::serde::json::Json;
use rusqlite::Connection;

/// Roles for board collaborators (ordered by privilege level)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoardRole {
    Viewer = 0,
    Editor = 1,
    Admin = 2,
    Owner = 3,
}

impl BoardRole {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "viewer" => Some(BoardRole::Viewer),
            "editor" => Some(BoardRole::Editor),
            "admin" => Some(BoardRole::Admin),
            "owner" => Some(BoardRole::Owner),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BoardRole::Viewer => "viewer",
            BoardRole::Editor => "editor",
            BoardRole::Admin => "admin",
            BoardRole::Owner => "owner",
        }
    }
}

/// Check what role a key has on a board. Returns None if no access.
pub fn get_board_role(
    conn: &Connection,
    board_id: &str,
    key: &AuthenticatedKey,
) -> Option<BoardRole> {
    // Global admin keys have full access to all boards
    if key.is_admin {
        return Some(BoardRole::Admin);
    }

    // Check if owner
    let is_owner: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM boards WHERE id = ?1 AND owner_key_id = ?2",
            rusqlite::params![board_id, key.id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if is_owner {
        return Some(BoardRole::Owner);
    }

    // Check collaborator role
    let role_str: Option<String> = conn
        .query_row(
            "SELECT role FROM board_collaborators WHERE board_id = ?1 AND key_id = ?2",
            rusqlite::params![board_id, key.id],
            |row| row.get(0),
        )
        .ok();

    role_str.and_then(|r| BoardRole::parse(&r))
}

/// Require at least the given role. Returns error if insufficient access.
pub fn require_role(
    conn: &Connection,
    board_id: &str,
    key: &AuthenticatedKey,
    min_role: BoardRole,
) -> Result<BoardRole, (Status, Json<ApiError>)> {
    // First check if board exists
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM boards WHERE id = ?1",
            rusqlite::params![board_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !exists {
        return Err((
            Status::NotFound,
            Json(ApiError {
                error: "Board not found".to_string(),
                code: "NOT_FOUND".to_string(),
                status: 404,
            }),
        ));
    }

    match get_board_role(conn, board_id, key) {
        Some(role) if role >= min_role => Ok(role),
        Some(_) => Err((
            Status::Forbidden,
            Json(ApiError {
                error: format!(
                    "Insufficient permissions. Required: {} or higher",
                    min_role.as_str()
                ),
                code: "INSUFFICIENT_ROLE".to_string(),
                status: 403,
            }),
        )),
        None => Err((
            Status::Forbidden,
            Json(ApiError {
                error: "You don't have access to this board".to_string(),
                code: "NO_ACCESS".to_string(),
                status: 403,
            }),
        )),
    }
}
