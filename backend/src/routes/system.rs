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

/// GET /SKILL.md — canonical AI-readable service guide
#[get("/SKILL.md")]
pub fn skill_md() -> (ContentType, &'static str) {
    (ContentType::Plain, include_str!("../../SKILL.md"))
}

#[get("/llms.txt")]
pub fn llms_txt() -> (ContentType, &'static str) {
    (ContentType::Plain, include_str!("../../SKILL.md"))
}

/// Root-level /llms.txt for standard discovery (outside /api/v1)
#[get("/llms.txt", rank = 2)]
pub fn root_llms_txt() -> (ContentType, &'static str) {
    (ContentType::Plain, include_str!("../../SKILL.md"))
}

// ── Well-Known Skills Discovery (Cloudflare RFC) ──

#[get("/.well-known/skills/index.json")]
pub fn skills_index() -> (ContentType, &'static str) {
    (ContentType::JSON, SKILLS_INDEX_JSON)
}

#[get("/.well-known/skills/kanban/SKILL.md")]
pub fn skills_skill_md() -> (ContentType, &'static str) {
    (ContentType::Plain, include_str!("../../SKILL.md"))
}

/// GET /skills/SKILL.md — alternate path for agent discoverability
#[get("/skills/SKILL.md")]
pub fn api_skills_skill_md() -> (ContentType, &'static str) {
    (ContentType::Plain, include_str!("../../SKILL.md"))
}

const SKILLS_INDEX_JSON: &str = r#"{
  "skills": [
    {
      "name": "kanban",
      "description": "Integrate with Kanban — a zero-signup project management board for AI agents. Create boards, manage tasks with drag-and-drop columns, track activity via SSE, and coordinate work across agents.",
      "url": "/SKILL.md",
      "files": [
        "SKILL.md"
      ]
    }
  ]
}"#;

// SKILL_MD constant removed — now served via include_str!("../../SKILL.md")

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
