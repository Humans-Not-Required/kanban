use std::path::PathBuf;

use rocket::http::ContentType;
use rocket::serde::json::Json;

use crate::models::HealthResponse;

#[get("/health")]
pub fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[get("/openapi.json")]
pub fn openapi() -> (ContentType, &'static str) {
    (ContentType::JSON, include_str!("../../openapi.json"))
}

#[get("/llms.txt")]
pub fn llms_txt() -> (ContentType, &'static str) {
    (ContentType::Text, include_str!("../../llms.txt"))
}

/// Root-level /llms.txt for standard discovery (outside /api/v1)
#[get("/llms.txt", rank = 2)]
pub fn root_llms_txt() -> (ContentType, &'static str) {
    (ContentType::Text, include_str!("../../llms.txt"))
}

// ============ SPA Fallback ============

#[get("/<_path..>", rank = 20)]
pub fn spa_fallback(_path: PathBuf) -> Option<(ContentType, Vec<u8>)> {
    let static_dir: PathBuf = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));
    let index_path = static_dir.join("index.html");
    std::fs::read(&index_path)
        .ok()
        .map(|bytes| (ContentType::HTML, bytes))
}
