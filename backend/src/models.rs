use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize priority from either an integer or a string like "low", "medium", "high", "critical".
fn deserialize_priority<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => n.as_i64().map(|v| v as i32).ok_or_else(|| serde::de::Error::custom("invalid number")),
        serde_json::Value::String(s) => match s.to_lowercase().as_str() {
            "critical" | "urgent" => Ok(3),
            "high" => Ok(2),
            "medium" | "normal" => Ok(1),
            "low" | "none" => Ok(0),
            other => other.parse::<i32>().map_err(|_| serde::de::Error::custom(format!("unknown priority: {}", other))),
        },
        serde_json::Value::Null => Ok(0),
        _ => Err(serde::de::Error::custom("priority must be a number or string")),
    }
}

/// Deserialize a String that accepts null as empty string.
fn deserialize_string_or_null<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

// ============ Boards ============

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBoardRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Optional initial columns. If omitted, creates default: Backlog, Up Next, In Progress, Review, Done
    #[serde(default)]
    pub columns: Vec<String>,
    /// Optional: make the board publicly listed (default: false = unlisted)
    #[serde(default)]
    pub is_public: bool,
    /// Require display name on tasks and comments (default: false = allow anonymous)
    #[serde(default)]
    pub require_display_name: bool,
}

/// Update board settings (all fields optional).
#[derive(Debug, Deserialize)]
pub struct UpdateBoardRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_public: Option<bool>,
    pub require_display_name: Option<bool>,
    pub quick_done_column_id: Option<String>,
    pub quick_done_auto_archive: Option<bool>,
    pub quick_reassign_column_id: Option<String>,
    pub quick_reassign_to: Option<String>,
}

/// Returned when creating a board. Includes the manage_key (shown only once).
#[derive(Debug, Serialize)]
pub struct CreateBoardResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub columns: Vec<ColumnResponse>,
    pub manage_key: String,
    pub view_url: String,
    pub manage_url: String,
    pub api_base: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct BoardResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub columns: Vec<ColumnResponse>,
    pub task_count: usize,
    pub archived: bool,
    pub is_public: bool,
    pub require_display_name: bool,
    pub quick_done_column_id: Option<String>,
    pub quick_done_auto_archive: bool,
    pub quick_reassign_column_id: Option<String>,
    pub quick_reassign_to: Option<String>,
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
    pub is_public: bool,
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

#[derive(Debug, Deserialize)]
pub struct UpdateColumnRequest {
    pub name: Option<String>,
    pub wip_limit: Option<Option<i32>>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderColumnsRequest {
    /// Ordered list of column IDs — first = position 0, second = position 1, etc.
    pub column_ids: Vec<String>,
}

// ============ Tasks ============

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_null")]
    pub description: String,
    /// Column ID. If omitted, uses the first column of the board.
    pub column_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_priority")]
    pub priority: i32,
    /// Explicit position within column. If omitted, appends to end.
    pub position: Option<i32>,
    pub assigned_to: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    /// Arbitrary JSON metadata for agent-specific data
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    pub due_at: Option<String>,
    /// Optional: identify who created this task (free text, e.g. "nanook", "jordan")
    #[serde(default, deserialize_with = "deserialize_string_or_null")]
    pub actor_name: String,
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
    /// Optional: identify who made this update
    #[serde(default)]
    pub actor_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderTaskRequest {
    /// New position (0-indexed). Tasks at and after this position shift down.
    pub position: i32,
    /// Optional: move to a different column at the same time.
    pub column_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
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
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub comment_count: i64,
}

#[derive(Debug, Serialize)]
pub struct TaskEventResponse {
    pub id: String,
    pub event_type: String,
    pub actor: String,
    pub data: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct BoardActivityItem {
    pub id: String,
    pub task_id: String,
    pub task_title: String,
    pub event_type: String,
    pub actor: String,
    pub data: serde_json::Value,
    pub created_at: String,
    /// Monotonic sequence number for cursor-based pagination (use `?after=<seq>`)
    pub seq: i64,
    /// Full task snapshot — included on `created` and `comment` events only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskResponse>,
    /// Recent comments (newest first, up to 10) — included on `comment` events only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_comments: Option<Vec<CommentSnapshot>>,
    /// @mentions extracted from comment text. Present on `comment` events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions: Option<Vec<String>>,
}

/// Lightweight comment representation for activity feed enrichment.
#[derive(Debug, Serialize, Clone)]
pub struct CommentSnapshot {
    pub id: String,
    pub actor: String,
    pub message: String,
    pub created_at: String,
}

// ============ Search ============

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub tasks: Vec<TaskResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

// ============ Batch Operations ============

#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    /// List of operations to perform. Max 50 per request.
    pub operations: Vec<BatchOperation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
pub enum BatchOperation {
    /// Move tasks to a different column
    #[serde(rename = "move")]
    Move {
        task_ids: Vec<String>,
        column_id: String,
    },
    /// Update fields on multiple tasks
    #[serde(rename = "update")]
    Update {
        task_ids: Vec<String>,
        #[serde(flatten)]
        fields: BatchUpdateFields,
    },
    /// Delete multiple tasks
    #[serde(rename = "delete")]
    Delete { task_ids: Vec<String> },
}

#[derive(Debug, Deserialize)]
pub struct BatchUpdateFields {
    pub priority: Option<i32>,
    pub assigned_to: Option<String>,
    pub labels: Option<Vec<String>>,
    pub due_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    /// Total operations submitted
    pub total: usize,
    /// Successfully completed
    pub succeeded: usize,
    /// Failed operations
    pub failed: usize,
    /// Per-operation results
    pub results: Vec<BatchOperationResult>,
}

#[derive(Debug, Serialize)]
pub struct BatchOperationResult {
    pub action: String,
    pub task_ids: Vec<String>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Number of tasks affected in this operation
    pub affected: usize,
}

// ============ Webhooks ============

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    /// URL to POST events to (must be HTTPS in production)
    pub url: String,
    /// Optional filter: list of event types to subscribe to.
    /// If empty, all events are delivered.
    #[serde(default)]
    pub events: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub url: Option<String>,
    pub events: Option<Vec<String>>,
    pub active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub id: String,
    pub board_id: String,
    pub url: String,
    /// Only returned on creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    pub events: Vec<String>,
    pub active: bool,
    pub failure_count: i32,
    pub last_triggered_at: Option<String>,
    pub created_at: String,
}

// ============ Task Dependencies ============

#[derive(Debug, Deserialize)]
pub struct CreateDependencyRequest {
    /// The task that blocks (must be completed first)
    pub blocker_task_id: String,
    /// The task that is blocked (cannot proceed until blocker is done)
    pub blocked_task_id: String,
    /// Optional note explaining the dependency
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct DependencyResponse {
    pub id: String,
    pub board_id: String,
    pub blocker_task_id: String,
    pub blocker_title: String,
    pub blocker_column: String,
    pub blocker_completed: bool,
    pub blocked_task_id: String,
    pub blocked_title: String,
    pub blocked_column: String,
    pub note: String,
    pub created_by: String,
    pub created_at: String,
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
