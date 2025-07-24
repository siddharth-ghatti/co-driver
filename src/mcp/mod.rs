pub mod client;
pub mod protocol;
pub mod server;
pub mod transport;

pub use client::McpClient;
pub use protocol::*;
pub use server::{DefaultMessageRouter, McpServer};
pub use transport::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// MCP Protocol version
pub const MCP_VERSION: &str = "2024-11-05";

/// JSON-RPC 2.0 version
pub const JSONRPC_VERSION: &str = "2.0";

/// Core MCP message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "jsonrpc")]
pub enum McpMessage {
    #[serde(rename = "2.0")]
    Request(McpRequest),
    #[serde(rename = "2.0")]
    Response(McpResponse),
    #[serde(rename = "2.0")]
    Notification(McpNotification),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub id: serde_json::Value,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNotification {
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Standard MCP error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // MCP-specific error codes
    pub const TOOL_NOT_FOUND: i32 = -32000;
    pub const RESOURCE_NOT_FOUND: i32 = -32001;
    pub const UNAUTHORIZED: i32 = -32002;
    pub const FORBIDDEN: i32 = -32003;
    pub const RATE_LIMITED: i32 = -32004;
    pub const TIMEOUT: i32 = -32005;
    pub const SERVER_ERROR: i32 = -32006;
}

/// MCP method names
pub mod methods {
    // Core protocol methods
    pub const INITIALIZE: &str = "initialize";
    pub const PING: &str = "ping";
    pub const SHUTDOWN: &str = "shutdown";

    // Tool methods
    pub const TOOLS_LIST: &str = "tools/list";
    pub const TOOLS_CALL: &str = "tools/call";

    // Resource methods
    pub const RESOURCES_LIST: &str = "resources/list";
    pub const RESOURCES_READ: &str = "resources/read";
    pub const RESOURCES_WRITE: &str = "resources/write";
    pub const RESOURCES_DELETE: &str = "resources/delete";

    // Prompt methods
    pub const PROMPTS_LIST: &str = "prompts/list";
    pub const PROMPTS_GET: &str = "prompts/get";

    // Notification methods
    pub const NOTIFICATIONS_TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
    pub const NOTIFICATIONS_RESOURCES_LIST_CHANGED: &str = "notifications/resources/list_changed";
    pub const NOTIFICATIONS_PROMPTS_LIST_CHANGED: &str = "notifications/prompts/list_changed";
}

/// Initialize request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapabilities>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingCapabilities {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Initialize response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapabilities>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingCapabilities {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapabilities {
    #[serde(rename = "listChanged")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapabilities {
    #[serde(rename = "listChanged")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapabilities {
    #[serde(rename = "listChanged")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Tool-related types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsListResult {
    pub tools: Vec<Tool>,
    #[serde(rename = "nextCursor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: ResourceReference },
}

/// Resource-related types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReference {
    pub uri: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesListResult {
    pub resources: Vec<Resource>,
    #[serde(rename = "nextCursor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesReadParams {
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesReadResult {
    pub contents: Vec<ResourceContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResourceContent {
    #[serde(rename = "text")]
    Text {
        uri: String,
        text: String,
        mime_type: Option<String>,
    },
    #[serde(rename = "blob")]
    Blob {
        uri: String,
        blob: String,
        mime_type: Option<String>,
    },
}

/// Prompt-related types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsListResult {
    pub prompts: Vec<Prompt>,
    #[serde(rename = "nextCursor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsGetParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsGetResult {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: PromptRole,
    pub content: PromptContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PromptContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: ResourceReference },
}

/// Utility functions
impl McpMessage {
    pub fn new_request(
        id: serde_json::Value,
        method: String,
        params: Option<serde_json::Value>,
    ) -> Self {
        McpMessage::Request(McpRequest { id, method, params })
    }

    pub fn new_response(id: serde_json::Value, result: Option<serde_json::Value>) -> Self {
        McpMessage::Response(McpResponse {
            id,
            result,
            error: None,
        })
    }

    pub fn new_error_response(id: serde_json::Value, error: McpError) -> Self {
        McpMessage::Response(McpResponse {
            id,
            result: None,
            error: Some(error),
        })
    }

    pub fn new_notification(method: String, params: Option<serde_json::Value>) -> Self {
        McpMessage::Notification(McpNotification { method, params })
    }

    pub fn is_request(&self) -> bool {
        matches!(self, McpMessage::Request(_))
    }

    pub fn is_response(&self) -> bool {
        matches!(self, McpMessage::Response(_))
    }

    pub fn is_notification(&self) -> bool {
        matches!(self, McpMessage::Notification(_))
    }

    pub fn method(&self) -> Option<&str> {
        match self {
            McpMessage::Request(req) => Some(&req.method),
            McpMessage::Notification(notif) => Some(&notif.method),
            _ => None,
        }
    }

    pub fn id(&self) -> Option<&serde_json::Value> {
        match self {
            McpMessage::Request(req) => Some(&req.id),
            McpMessage::Response(resp) => Some(&resp.id),
            _ => None,
        }
    }
}

impl McpError {
    pub fn new(code: i32, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }

    pub fn with_data(code: i32, message: String, data: serde_json::Value) -> Self {
        Self {
            code,
            message,
            data: Some(data),
        }
    }

    pub fn parse_error() -> Self {
        Self::new(error_codes::PARSE_ERROR, "Parse error".to_string())
    }

    pub fn invalid_request() -> Self {
        Self::new(error_codes::INVALID_REQUEST, "Invalid request".to_string())
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(
            error_codes::METHOD_NOT_FOUND,
            format!("Method '{}' not found", method),
        )
    }

    pub fn invalid_params(message: &str) -> Self {
        Self::new(
            error_codes::INVALID_PARAMS,
            format!("Invalid params: {}", message),
        )
    }

    pub fn internal_error(message: &str) -> Self {
        Self::new(
            error_codes::INTERNAL_ERROR,
            format!("Internal error: {}", message),
        )
    }

    pub fn tool_not_found(tool_name: &str) -> Self {
        Self::new(
            error_codes::TOOL_NOT_FOUND,
            format!("Tool '{}' not found", tool_name),
        )
    }

    pub fn resource_not_found(uri: &str) -> Self {
        Self::new(
            error_codes::RESOURCE_NOT_FOUND,
            format!("Resource '{}' not found", uri),
        )
    }

    pub fn unauthorized() -> Self {
        Self::new(error_codes::UNAUTHORIZED, "Unauthorized".to_string())
    }

    pub fn forbidden() -> Self {
        Self::new(error_codes::FORBIDDEN, "Forbidden".to_string())
    }

    pub fn rate_limited() -> Self {
        Self::new(error_codes::RATE_LIMITED, "Rate limited".to_string())
    }

    pub fn timeout() -> Self {
        Self::new(error_codes::TIMEOUT, "Request timeout".to_string())
    }

    pub fn server_error(message: &str) -> Self {
        Self::new(
            error_codes::SERVER_ERROR,
            format!("Server error: {}", message),
        )
    }
}

/// Generate a unique request ID
pub fn generate_request_id() -> serde_json::Value {
    serde_json::Value::String(Uuid::new_v4().to_string())
}

/// Check if a method is a notification method
pub fn is_notification_method(method: &str) -> bool {
    method.starts_with("notifications/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_message_serialization() {
        let request = McpMessage::new_request(
            serde_json::Value::String("123".to_string()),
            "tools/list".to_string(),
            None,
        );

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: McpMessage = serde_json::from_str(&json).unwrap();

        assert!(deserialized.is_request());
        assert_eq!(deserialized.method(), Some("tools/list"));
    }

    #[test]
    fn test_mcp_error_creation() {
        let error = McpError::tool_not_found("test_tool");
        assert_eq!(error.code, error_codes::TOOL_NOT_FOUND);
        assert!(error.message.contains("test_tool"));
    }

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_notification_method_detection() {
        assert!(is_notification_method("notifications/tools/list_changed"));
        assert!(!is_notification_method("tools/list"));
    }
}
