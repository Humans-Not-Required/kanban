use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};

/// Extracts a board management token from the request.
/// Checks (in order):
///   1. `Authorization: Bearer <token>` header
///   2. `X-API-Key` header
///   3. `?key=<token>` query parameter
///
/// The token is NOT validated here — it's just extracted.
/// Route handlers call `require_manage_key()` to verify against a specific board.
#[derive(Debug, Clone)]
pub struct BoardToken(pub String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BoardToken {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // 1. Authorization: Bearer header
        if let Some(auth) = request.headers().get_one("Authorization") {
            if let Some(key) = auth.strip_prefix("Bearer ") {
                if !key.is_empty() {
                    return Outcome::Success(BoardToken(key.to_string()));
                }
            }
            return Outcome::Error((
                Status::Unauthorized,
                "Invalid authorization format. Use: Bearer YOUR_MANAGE_KEY",
            ));
        }

        // 2. X-API-Key header
        if let Some(key) = request.headers().get_one("X-API-Key") {
            if !key.is_empty() {
                return Outcome::Success(BoardToken(key.to_string()));
            }
        }

        // 3. ?key= query parameter
        if let Some(Ok(k)) = request.query_value::<String>("key") {
            if !k.is_empty() {
                return Outcome::Success(BoardToken(k));
            }
        }

        Outcome::Error((
            Status::Unauthorized,
            "Missing management key. Use Authorization: Bearer YOUR_KEY, X-API-Key header, or ?key= query param",
        ))
    }
}

// Note: OptionalBoardToken and helper functions can be added later if needed
// for routes that optionally detect management access.
