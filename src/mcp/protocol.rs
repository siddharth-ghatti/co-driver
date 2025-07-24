use crate::error::{GatewayError, Result};
use crate::mcp::*;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// MCP Protocol handler
pub struct McpProtocol {
    capabilities: ServerCapabilities,
    server_info: ServerInfo,
    initialized: bool,
}

impl McpProtocol {
    pub fn new(name: String, version: String) -> Self {
        Self {
            capabilities: ServerCapabilities {
                experimental: None,
                logging: Some(LoggingCapabilities {}),
                prompts: Some(PromptsCapabilities {
                    list_changed: Some(true),
                }),
                resources: Some(ResourcesCapabilities {
                    list_changed: Some(true),
                    subscribe: Some(false),
                }),
                tools: Some(ToolsCapabilities {
                    list_changed: Some(true),
                }),
            },
            server_info: ServerInfo { name, version },
            initialized: false,
        }
    }

    /// Process an incoming MCP message
    pub async fn process_message(&mut self, message: McpMessage) -> Result<Option<McpMessage>> {
        debug!("Processing MCP message: {:?}", message);

        match message {
            McpMessage::Request(request) => self.handle_request(request).await,
            McpMessage::Response(response) => {
                // Handle responses from upstream servers
                self.handle_response(response).await
            }
            McpMessage::Notification(notification) => self.handle_notification(notification).await,
        }
    }

    async fn handle_request(&mut self, request: McpRequest) -> Result<Option<McpMessage>> {
        let method = &request.method;
        let id = request.id.clone();

        info!("Handling MCP request: {}", method);

        let result = match method.as_str() {
            methods::INITIALIZE => self.handle_initialize(request.params).await,
            methods::PING => self.handle_ping().await,
            methods::SHUTDOWN => self.handle_shutdown().await,
            methods::TOOLS_LIST => self.handle_tools_list(request.params).await,
            methods::TOOLS_CALL => self.handle_tools_call(request.params).await,
            methods::RESOURCES_LIST => self.handle_resources_list(request.params).await,
            methods::RESOURCES_READ => self.handle_resources_read(request.params).await,
            methods::PROMPTS_LIST => self.handle_prompts_list(request.params).await,
            methods::PROMPTS_GET => self.handle_prompts_get(request.params).await,
            _ => {
                warn!("Unknown method: {}", method);
                Err(GatewayError::McpProtocol(format!(
                    "Method '{}' not found",
                    method
                )))
            }
        };

        match result {
            Ok(response_data) => Ok(Some(McpMessage::new_response(id, Some(response_data)))),
            Err(err) => {
                error!("Error handling request '{}': {}", method, err);
                let mcp_error = self.gateway_error_to_mcp_error(err);
                Ok(Some(McpMessage::new_error_response(id, mcp_error)))
            }
        }
    }

    async fn handle_response(&self, _response: McpResponse) -> Result<Option<McpMessage>> {
        // In a gateway, we typically forward responses back to the original client
        // This is handled by the gateway's routing logic
        Ok(None)
    }

    async fn handle_notification(
        &self,
        notification: McpNotification,
    ) -> Result<Option<McpMessage>> {
        info!("Handling notification: {}", notification.method);

        match notification.method.as_str() {
            methods::NOTIFICATIONS_TOOLS_LIST_CHANGED => {
                self.handle_tools_list_changed_notification(notification.params)
                    .await
            }
            methods::NOTIFICATIONS_RESOURCES_LIST_CHANGED => {
                self.handle_resources_list_changed_notification(notification.params)
                    .await
            }
            methods::NOTIFICATIONS_PROMPTS_LIST_CHANGED => {
                self.handle_prompts_list_changed_notification(notification.params)
                    .await
            }
            _ => {
                warn!("Unknown notification method: {}", notification.method);
                Ok(None)
            }
        }
    }

    async fn handle_initialize(&mut self, params: Option<Value>) -> Result<Value> {
        let params: InitializeParams = if let Some(p) = params {
            serde_json::from_value(p).map_err(|e| {
                GatewayError::McpProtocol(format!("Invalid initialize params: {}", e))
            })?
        } else {
            return Err(GatewayError::McpProtocol(
                "Initialize params required".to_string(),
            ));
        };

        info!(
            "Initializing MCP connection with client: {} v{}",
            params.client_info.name, params.client_info.version
        );

        // Validate protocol version
        if params.protocol_version != MCP_VERSION {
            warn!(
                "Protocol version mismatch: client={}, server={}",
                params.protocol_version, MCP_VERSION
            );
        }

        self.initialized = true;

        let result = InitializeResult {
            protocol_version: MCP_VERSION.to_string(),
            capabilities: self.capabilities.clone(),
            server_info: self.server_info.clone(),
        };

        serde_json::to_value(result).map_err(|e| {
            GatewayError::McpProtocol(format!("Failed to serialize initialize result: {}", e))
        })
    }

    async fn handle_ping(&self) -> Result<Value> {
        if !self.initialized {
            return Err(GatewayError::McpProtocol("Not initialized".to_string()));
        }

        debug!("Handling ping request");
        Ok(Value::Object(serde_json::Map::new()))
    }

    async fn handle_shutdown(&mut self) -> Result<Value> {
        info!("Handling shutdown request");
        self.initialized = false;
        Ok(Value::Object(serde_json::Map::new()))
    }

    async fn handle_tools_list(&self, params: Option<Value>) -> Result<Value> {
        self.check_initialized()?;

        let _params: Option<ToolsListParams> = if let Some(p) = params {
            Some(serde_json::from_value(p).map_err(|e| {
                GatewayError::McpProtocol(format!("Invalid tools/list params: {}", e))
            })?)
        } else {
            None
        };

        // This would be implemented by the gateway to aggregate tools from all upstream servers
        let result = ToolsListResult {
            tools: vec![],
            next_cursor: None,
        };

        serde_json::to_value(result).map_err(|e| {
            GatewayError::McpProtocol(format!("Failed to serialize tools list: {}", e))
        })
    }

    async fn handle_tools_call(&self, params: Option<Value>) -> Result<Value> {
        self.check_initialized()?;

        let params: ToolCallParams = if let Some(p) = params {
            serde_json::from_value(p).map_err(|e| {
                GatewayError::McpProtocol(format!("Invalid tools/call params: {}", e))
            })?
        } else {
            return Err(GatewayError::McpProtocol(
                "Tool call params required".to_string(),
            ));
        };

        info!("Tool call request for: {}", params.name);

        // This would be forwarded to the appropriate upstream server
        let result = ToolCallResult {
            content: vec![ToolContent::Text {
                text: "Tool call forwarded to upstream server".to_string(),
            }],
            is_error: Some(false),
        };

        serde_json::to_value(result).map_err(|e| {
            GatewayError::McpProtocol(format!("Failed to serialize tool call result: {}", e))
        })
    }

    async fn handle_resources_list(&self, params: Option<Value>) -> Result<Value> {
        self.check_initialized()?;

        let _params: Option<ResourcesListParams> = if let Some(p) = params {
            Some(serde_json::from_value(p).map_err(|e| {
                GatewayError::McpProtocol(format!("Invalid resources/list params: {}", e))
            })?)
        } else {
            None
        };

        let result = ResourcesListResult {
            resources: vec![],
            next_cursor: None,
        };

        serde_json::to_value(result).map_err(|e| {
            GatewayError::McpProtocol(format!("Failed to serialize resources list: {}", e))
        })
    }

    async fn handle_resources_read(&self, params: Option<Value>) -> Result<Value> {
        self.check_initialized()?;

        let params: ResourcesReadParams = if let Some(p) = params {
            serde_json::from_value(p).map_err(|e| {
                GatewayError::McpProtocol(format!("Invalid resources/read params: {}", e))
            })?
        } else {
            return Err(GatewayError::McpProtocol(
                "Resource read params required".to_string(),
            ));
        };

        info!("Resource read request for: {}", params.uri);

        let result = ResourcesReadResult { contents: vec![] };

        serde_json::to_value(result).map_err(|e| {
            GatewayError::McpProtocol(format!("Failed to serialize resource read result: {}", e))
        })
    }

    async fn handle_prompts_list(&self, params: Option<Value>) -> Result<Value> {
        self.check_initialized()?;

        let _params: Option<PromptsListParams> = if let Some(p) = params {
            Some(serde_json::from_value(p).map_err(|e| {
                GatewayError::McpProtocol(format!("Invalid prompts/list params: {}", e))
            })?)
        } else {
            None
        };

        let result = PromptsListResult {
            prompts: vec![],
            next_cursor: None,
        };

        serde_json::to_value(result).map_err(|e| {
            GatewayError::McpProtocol(format!("Failed to serialize prompts list: {}", e))
        })
    }

    async fn handle_prompts_get(&self, params: Option<Value>) -> Result<Value> {
        self.check_initialized()?;

        let params: PromptsGetParams = if let Some(p) = params {
            serde_json::from_value(p).map_err(|e| {
                GatewayError::McpProtocol(format!("Invalid prompts/get params: {}", e))
            })?
        } else {
            return Err(GatewayError::McpProtocol(
                "Prompt get params required".to_string(),
            ));
        };

        info!("Prompt get request for: {}", params.name);

        let result = PromptsGetResult {
            description: Some("Gateway-managed prompt".to_string()),
            messages: vec![],
        };

        serde_json::to_value(result).map_err(|e| {
            GatewayError::McpProtocol(format!("Failed to serialize prompt get result: {}", e))
        })
    }

    async fn handle_tools_list_changed_notification(
        &self,
        _params: Option<Value>,
    ) -> Result<Option<McpMessage>> {
        info!("Tools list changed notification received");
        // Forward to connected clients
        Ok(None)
    }

    async fn handle_resources_list_changed_notification(
        &self,
        _params: Option<Value>,
    ) -> Result<Option<McpMessage>> {
        info!("Resources list changed notification received");
        // Forward to connected clients
        Ok(None)
    }

    async fn handle_prompts_list_changed_notification(
        &self,
        _params: Option<Value>,
    ) -> Result<Option<McpMessage>> {
        info!("Prompts list changed notification received");
        // Forward to connected clients
        Ok(None)
    }

    fn check_initialized(&self) -> Result<()> {
        if !self.initialized {
            return Err(GatewayError::McpProtocol(
                "Connection not initialized".to_string(),
            ));
        }
        Ok(())
    }

    fn gateway_error_to_mcp_error(&self, error: GatewayError) -> McpError {
        match error {
            GatewayError::McpProtocol(msg) => McpError::invalid_request(),
            GatewayError::Authentication(_) => McpError::unauthorized(),
            GatewayError::Authorization(_) => McpError::forbidden(),
            GatewayError::RateLimit => McpError::rate_limited(),
            GatewayError::Timeout(_) => McpError::timeout(),
            GatewayError::NotFound(resource) => {
                if resource.starts_with("tool:") {
                    McpError::tool_not_found(&resource[5..])
                } else if resource.starts_with("resource:") {
                    McpError::resource_not_found(&resource[9..])
                } else {
                    McpError::internal_error(&format!("Resource not found: {}", resource))
                }
            }
            _ => McpError::internal_error(&error.to_string()),
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    pub fn server_info(&self) -> &ServerInfo {
        &self.server_info
    }
}

/// Protocol validation utilities
pub fn validate_message(message: &McpMessage) -> Result<()> {
    match message {
        McpMessage::Request(req) => validate_request(req),
        McpMessage::Response(resp) => validate_response(resp),
        McpMessage::Notification(notif) => validate_notification(notif),
    }
}

fn validate_request(request: &McpRequest) -> Result<()> {
    if request.method.is_empty() {
        return Err(GatewayError::McpProtocol(
            "Request method cannot be empty".to_string(),
        ));
    }

    // Validate specific method requirements
    match request.method.as_str() {
        methods::INITIALIZE
        | methods::TOOLS_CALL
        | methods::RESOURCES_READ
        | methods::PROMPTS_GET => {
            if request.params.is_none() {
                return Err(GatewayError::McpProtocol(format!(
                    "Method '{}' requires parameters",
                    request.method
                )));
            }
        }
        _ => {}
    }

    Ok(())
}

fn validate_response(response: &McpResponse) -> Result<()> {
    if response.result.is_some() && response.error.is_some() {
        return Err(GatewayError::McpProtocol(
            "Response cannot have both result and error".to_string(),
        ));
    }

    if response.result.is_none() && response.error.is_none() {
        return Err(GatewayError::McpProtocol(
            "Response must have either result or error".to_string(),
        ));
    }

    Ok(())
}

fn validate_notification(notification: &McpNotification) -> Result<()> {
    if notification.method.is_empty() {
        return Err(GatewayError::McpProtocol(
            "Notification method cannot be empty".to_string(),
        ));
    }

    if !is_notification_method(&notification.method) {
        return Err(GatewayError::McpProtocol(format!(
            "Invalid notification method: {}",
            notification.method
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_protocol_initialization() {
        let mut protocol = McpProtocol::new("test-gateway".to_string(), "1.0.0".to_string());
        assert!(!protocol.is_initialized());

        let init_params = InitializeParams {
            protocol_version: MCP_VERSION.to_string(),
            capabilities: ClientCapabilities {
                experimental: None,
                sampling: None,
            },
            client_info: ClientInfo {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        };

        let request = McpMessage::new_request(
            generate_request_id(),
            methods::INITIALIZE.to_string(),
            Some(serde_json::to_value(init_params).unwrap()),
        );

        let response = protocol.process_message(request).await.unwrap();
        assert!(response.is_some());
        assert!(protocol.is_initialized());
    }

    #[tokio::test]
    async fn test_ping_without_initialization() {
        let mut protocol = McpProtocol::new("test-gateway".to_string(), "1.0.0".to_string());

        let request =
            McpMessage::new_request(generate_request_id(), methods::PING.to_string(), None);

        let response = protocol.process_message(request).await.unwrap();
        assert!(response.is_some());

        if let Some(McpMessage::Response(resp)) = response {
            assert!(resp.error.is_some());
        } else {
            panic!("Expected error response");
        }
    }

    #[test]
    fn test_message_validation() {
        let valid_request = McpRequest {
            id: generate_request_id(),
            method: "test/method".to_string(),
            params: None,
        };

        assert!(validate_request(&valid_request).is_ok());

        let invalid_request = McpRequest {
            id: generate_request_id(),
            method: "".to_string(),
            params: None,
        };

        assert!(validate_request(&invalid_request).is_err());
    }

    #[test]
    fn test_response_validation() {
        let valid_response = McpResponse {
            id: generate_request_id(),
            result: Some(Value::Null),
            error: None,
        };

        assert!(validate_response(&valid_response).is_ok());

        let invalid_response = McpResponse {
            id: generate_request_id(),
            result: Some(Value::Null),
            error: Some(McpError::internal_error("test")),
        };

        assert!(validate_response(&invalid_response).is_err());
    }
}
