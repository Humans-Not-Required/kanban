use serde::{Deserialize, Serialize};

// ============ Boards ============

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBoardRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Optional initial columns. If omitted, creates default: Backlog, In Progress, Done
    #[serde(default)]
    pub columns: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BoardResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner: String,
    pub columns: Vec<ColumnResponse>,
    pub task_count: usize,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct BoardSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub task_count: i64,
    pub archived: bool,
    pub created_at: String,
}

// ============ Columns ============

#[derive(Debug, Serialize)]
pub struct ColumnResponse {
    pub id: String,
    pub name: String,
    pub position: i32,
    pub wip_limit: Option<i32>,
    pub task_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateColumnRequest {
    pub name: String,
    pub position: Option<i32>,
    pub wip_limit: Option<i32>,
}

// ============ Tasks ============

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Column ID. If omitted, uses the first column of the board.
    pub column_id: Option<String>,
    #[serde(default)]
    pub priority: i32,
    pub assigned_to: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    /// Arbitrary JSON metadata for agent-specific data
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    pub due_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub column_id: Option<String>,
    pub priority: Option<i32>,
    pub assigned_to: Option<String>,
    pub labels: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
    pub due_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: String,
    pub board_id: String,
    pub column_id: String,
    pub column_name: String,
    pub title: String,
    pub description: String,
    pub priority: i32,
    pub position: i32,
    pub created_by: String,
    pub assigned_to: Option<String>,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<String>,
    pub labels: Vec<String>,
    pub metadata: serde_json::Value,
    pub due_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct TaskEventResponse {
    pub id: String,
    pub event_type: String,
    pub actor: String,
    pub data: serde_json::Value,
    pub created_at: String,
}

// ============ API Keys ============

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    /// Optional agent identifier (e.g. "nanook", "claude-agent-1")
    pub agent_id: Option<String>,
    #[serde(default = "default_rate_limit")]
    pub rate_limit: i64,
}

#[derive(Debug, Serialize)]
pub struct KeyResponse {
    pub id: String,
    pub name: String,
    pub agent_id: Option<String>,
    pub key: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub requests_count: i64,
    pub rate_limit: i64,
    pub active: bool,
}

// ============ Common ============

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
    pub status: u16,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

fn default_rate_limit() -> i64 {
    100
}
