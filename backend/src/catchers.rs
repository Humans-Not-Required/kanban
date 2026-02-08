use rocket::serde::json::Json;
use rocket::Request;
use serde_json::json;

#[catch(401)]
pub fn unauthorized(_req: &Request) -> Json<serde_json::Value> {
    Json(json!({
        "error": "UNAUTHORIZED",
        "message": "Missing or invalid management key. Use Authorization: Bearer YOUR_KEY, X-API-Key header, or ?key= query param."
    }))
}

#[catch(404)]
pub fn not_found(_req: &Request) -> Json<serde_json::Value> {
    Json(json!({
        "error": "NOT_FOUND",
        "message": "The requested resource was not found."
    }))
}

#[catch(422)]
pub fn unprocessable(_req: &Request) -> Json<serde_json::Value> {
    Json(json!({
        "error": "UNPROCESSABLE_ENTITY",
        "message": "The request body could not be processed."
    }))
}

#[catch(429)]
pub fn too_many_requests(_req: &Request) -> Json<serde_json::Value> {
    Json(json!({
        "error": "RATE_LIMIT_EXCEEDED",
        "message": "Too many requests. Please try again later."
    }))
}

#[catch(500)]
pub fn internal_error(_req: &Request) -> Json<serde_json::Value> {
    Json(json!({
        "error": "INTERNAL_ERROR",
        "message": "An internal server error occurred."
    }))
}
