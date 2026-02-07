use crate::db::{hash_key, DbPool};
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;

#[derive(Debug)]
pub struct AuthenticatedKey {
    pub id: String,
    #[allow(dead_code)]
    pub name: String,
    pub is_admin: bool,
    pub agent_id: Option<String>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedKey {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let db = match request.guard::<&State<DbPool>>().await {
            Outcome::Success(db) => db,
            _ => return Outcome::Error((Status::InternalServerError, "Database unavailable")),
        };

        // Check Authorization header, then X-API-Key
        let key =
            match request.headers().get_one("Authorization") {
                Some(auth) => {
                    if let Some(key) = auth.strip_prefix("Bearer ") {
                        key.to_string()
                    } else {
                        return Outcome::Error((
                            Status::Unauthorized,
                            "Invalid authorization format. Use: Bearer YOUR_API_KEY",
                        ));
                    }
                }
                None => match request.headers().get_one("X-API-Key") {
                    Some(key) => key.to_string(),
                    None => return Outcome::Error((
                        Status::Unauthorized,
                        "Missing API key. Use Authorization: Bearer YOUR_KEY or X-API-Key header",
                    )),
                },
            };

        let key_hash = hash_key(&key);
        let conn = db.lock().unwrap();

        match conn.query_row(
            "SELECT id, name, is_admin, agent_id FROM api_keys WHERE key_hash = ?1 AND active = 1",
            rusqlite::params![key_hash],
            |row| {
                Ok(AuthenticatedKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    is_admin: row.get::<_, i32>(2)? == 1,
                    agent_id: row.get(3)?,
                })
            },
        ) {
            Ok(auth_key) => {
                let _ = conn.execute(
                    "UPDATE api_keys SET last_used_at = datetime('now'), requests_count = requests_count + 1 WHERE id = ?1",
                    rusqlite::params![auth_key.id],
                );
                Outcome::Success(auth_key)
            }
            Err(_) => Outcome::Error((Status::Unauthorized, "Invalid API key")),
        }
    }
}
