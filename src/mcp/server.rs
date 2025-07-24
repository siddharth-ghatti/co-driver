use crate::error::{GatewayError, Result};
use crate::mcp::*;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// MCP server handler for incoming client connections
pub struct McpServer {
    server_id: String,
    protocol: Arc<RwLock<McpProtocol>>,
    active_connections: Arc<RwLock<HashMap<String, ConnectionHandle>>>,
    message_router: Arc<dyn MessageRouter + Send + Sync>,
}

/// Handle for an active MCP connection
pub struct ConnectionHandle {
    pub connection_id: String,
    pub client_info: Option<ClientInfo>,
    pub capabilities: Option<ClientCapabilities>,
    pub sender: mpsc::UnboundedSender<McpMessage>,
    pub initialized: bool,
}

/// Message router trait for handling MCP messages
#[async_trait::async_trait]
pub trait MessageRouter {
    async fn route_message(
        &self,
        connection_id: &str,
        message: McpMessage,
    ) -> Result<Option<McpMessage>>;
}

/// Default message router implementation
pub struct DefaultMessageRouter {
    upstream_clients: Arc<RwLock<HashMap<String, Arc<crate::mcp::McpClient>>>>,
}

impl McpServer {
    pub fn new(server_id: String, message_router: Arc<dyn MessageRouter + Send + Sync>) -> Self {
        Self {
            server_id: server_id.clone(),
            protocol: Arc::new(RwLock::new(McpProtocol::new(
                format!("{}-gateway", server_id),
                env!("CARGO_PKG_VERSION").to_string(),
            ))),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            message_router,
        }
    }

    /// Handle a new WebSocket connection
    pub async fn handle_connection(&self, websocket: WebSocket) -> Result<()> {
        let connection_id = Uuid::new_v4().to_string();
        info!("New MCP connection: {}", connection_id);

        let (mut ws_sender, mut ws_receiver) = websocket.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<McpMessage>();

        // Create connection handle
        let connection_handle = ConnectionHandle {
            connection_id: connection_id.clone(),
            client_info: None,
            capabilities: None,
            sender: tx,
            initialized: false,
        };

        // Add to active connections
        {
            let mut connections = self.active_connections.write().await;
            connections.insert(connection_id.clone(), connection_handle);
        }

        // Spawn task to handle outgoing messages
        let active_connections = self.active_connections.clone();
        let outgoing_connection_id = connection_id.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match serde_json::to_string(&message) {
                    Ok(json) => {
                        if let Err(e) = ws_sender.send(Message::Text(json)).await {
                            error!("Failed to send WebSocket message: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize message: {}", e);
                    }
                }
            }

            // Remove connection when sender task ends
            let mut connections = active_connections.write().await;
            connections.remove(&outgoing_connection_id);
            info!("Connection {} closed", outgoing_connection_id);
        });

        let mut pong_sender = ws_sender;

        // Handle incoming messages
        let server = self.clone();
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<McpMessage>(&text) {
                        Ok(mcp_message) => {
                            if let Err(e) = server.handle_message(&connection_id, mcp_message).await
                            {
                                error!("Error handling message: {}", e);
                                // Send error response if it was a request
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text)
                                {
                                    if let Some(id) = parsed.get("id") {
                                        let error_response = McpMessage::new_error_response(
                                            id.clone(),
                                            McpError::internal_error(&e.to_string()),
                                        );
                                        server
                                            .send_message(&connection_id, error_response)
                                            .await
                                            .ok();
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse MCP message: {}", e);
                            // Send parse error response
                            let error_response = McpMessage::new_error_response(
                                Value::Null,
                                McpError::parse_error(),
                            );
                            server
                                .send_message(&connection_id, error_response)
                                .await
                                .ok();
                        }
                    }
                }
                Ok(Message::Binary(_)) => {
                    warn!("Binary messages not supported");
                }
                Ok(Message::Ping(data)) => {
                    if let Err(e) = pong_sender.send(Message::Pong(data)).await {
                        error!("Failed to send pong: {}", e);
                        break;
                    }
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong from {}", connection_id);
                }
                Ok(Message::Close(_)) => {
                    info!("Connection {} closed by client", connection_id);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error for connection {}: {}", connection_id, e);
                    break;
                }
            }
        }

        // Clean up connection
        {
            let mut connections = self.active_connections.write().await;
            connections.remove(&connection_id);
        }

        Ok(())
    }

    async fn handle_message(&self, connection_id: &str, message: McpMessage) -> Result<()> {
        debug!(
            "Handling message for connection {}: {:?}",
            connection_id, message
        );

        // Validate message
        validate_message(&message)?;

        match &message {
            McpMessage::Request(request) => {
                self.handle_request(connection_id, request.clone()).await
            }
            McpMessage::Response(response) => {
                self.handle_response(connection_id, response.clone()).await
            }
            McpMessage::Notification(notification) => {
                self.handle_notification(connection_id, notification.clone())
                    .await
            }
        }
    }

    async fn handle_request(&self, connection_id: &str, request: McpRequest) -> Result<()> {
        let method = &request.method;
        let id = request.id.clone();

        match method.as_str() {
            methods::INITIALIZE => {
                let response = self
                    .handle_initialize_request(connection_id, request)
                    .await?;
                self.send_message(connection_id, response).await?;
            }
            methods::PING => {
                let response =
                    McpMessage::new_response(id, Some(Value::Object(serde_json::Map::new())));
                self.send_message(connection_id, response).await?;
            }
            methods::SHUTDOWN => {
                let response =
                    McpMessage::new_response(id, Some(Value::Object(serde_json::Map::new())));
                self.send_message(connection_id, response).await?;
                // Mark connection for cleanup
            }
            _ => {
                // Route to upstream servers via message router
                if let Some(response) = self
                    .message_router
                    .route_message(connection_id, McpMessage::Request(request))
                    .await?
                {
                    self.send_message(connection_id, response).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_response(&self, connection_id: &str, response: McpResponse) -> Result<()> {
        // Route response back to appropriate client
        if let Some(routed_response) = self
            .message_router
            .route_message(connection_id, McpMessage::Response(response))
            .await?
        {
            self.send_message(connection_id, routed_response).await?;
        }
        Ok(())
    }

    async fn handle_notification(
        &self,
        connection_id: &str,
        notification: McpNotification,
    ) -> Result<()> {
        // Route notification
        if let Some(routed_notification) = self
            .message_router
            .route_message(connection_id, McpMessage::Notification(notification))
            .await?
        {
            self.send_message(connection_id, routed_notification)
                .await?;
        }
        Ok(())
    }

    async fn handle_initialize_request(
        &self,
        connection_id: &str,
        request: McpRequest,
    ) -> Result<McpMessage> {
        let params: InitializeParams = if let Some(p) = request.params {
            serde_json::from_value(p).map_err(|e| {
                GatewayError::McpProtocol(format!("Invalid initialize params: {}", e))
            })?
        } else {
            return Ok(McpMessage::new_error_response(
                request.id,
                McpError::invalid_params("Initialize params required"),
            ));
        };

        info!(
            "Client {} initializing: {} v{}",
            connection_id, params.client_info.name, params.client_info.version
        );

        // Update connection with client info
        {
            let mut connections = self.active_connections.write().await;
            if let Some(connection) = connections.get_mut(connection_id) {
                connection.client_info = Some(params.client_info);
                connection.capabilities = Some(params.capabilities);
                connection.initialized = true;
            }
        }

        // Get server capabilities from protocol
        let protocol = self.protocol.read().await;
        let result = InitializeResult {
            protocol_version: MCP_VERSION.to_string(),
            capabilities: protocol.capabilities().clone(),
            server_info: protocol.server_info().clone(),
        };

        let result_value = serde_json::to_value(result).map_err(|e| {
            GatewayError::McpProtocol(format!("Failed to serialize initialize result: {}", e))
        })?;

        Ok(McpMessage::new_response(request.id, Some(result_value)))
    }

    async fn send_message(&self, connection_id: &str, message: McpMessage) -> Result<()> {
        let connections = self.active_connections.read().await;
        if let Some(connection) = connections.get(connection_id) {
            connection.sender.send(message).map_err(|_| {
                GatewayError::Internal(format!(
                    "Failed to send message to connection {}",
                    connection_id
                ))
            })?;
        } else {
            return Err(GatewayError::NotFound(format!(
                "Connection {}",
                connection_id
            )));
        }
        Ok(())
    }

    /// Broadcast a message to all connected clients
    pub async fn broadcast_message(&self, message: McpMessage) -> Result<()> {
        let connections = self.active_connections.read().await;
        for (connection_id, connection) in connections.iter() {
            if let Err(e) = connection.sender.send(message.clone()) {
                warn!("Failed to broadcast to connection {}: {}", connection_id, e);
            }
        }
        Ok(())
    }

    /// Send notification to specific connection
    pub async fn send_notification(
        &self,
        connection_id: &str,
        method: &str,
        params: Option<Value>,
    ) -> Result<()> {
        let notification = McpMessage::new_notification(method.to_string(), params);
        self.send_message(connection_id, notification).await
    }

    /// Broadcast notification to all connections
    pub async fn broadcast_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        let notification = McpMessage::new_notification(method.to_string(), params);
        self.broadcast_message(notification).await
    }

    /// Get active connection count
    pub async fn connection_count(&self) -> usize {
        self.active_connections.read().await.len()
    }

    /// Get connection info
    pub async fn get_connection_info(&self, connection_id: &str) -> Option<ConnectionInfo> {
        let connections = self.active_connections.read().await;
        connections.get(connection_id).map(|conn| ConnectionInfo {
            connection_id: conn.connection_id.clone(),
            client_info: conn.client_info.clone(),
            capabilities: conn.capabilities.clone(),
            initialized: conn.initialized,
        })
    }

    /// List all active connections
    pub async fn list_connections(&self) -> Vec<ConnectionInfo> {
        let connections = self.active_connections.read().await;
        connections
            .values()
            .map(|conn| ConnectionInfo {
                connection_id: conn.connection_id.clone(),
                client_info: conn.client_info.clone(),
                capabilities: conn.capabilities.clone(),
                initialized: conn.initialized,
            })
            .collect()
    }

    /// Close a specific connection
    pub async fn close_connection(&self, connection_id: &str) -> Result<()> {
        let mut connections = self.active_connections.write().await;
        if connections.remove(connection_id).is_some() {
            info!("Connection {} closed by server", connection_id);
            Ok(())
        } else {
            Err(GatewayError::NotFound(format!(
                "Connection {}",
                connection_id
            )))
        }
    }
}

impl Clone for McpServer {
    fn clone(&self) -> Self {
        Self {
            server_id: self.server_id.clone(),
            protocol: self.protocol.clone(),
            active_connections: self.active_connections.clone(),
            message_router: self.message_router.clone(),
        }
    }
}

/// Connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub connection_id: String,
    pub client_info: Option<ClientInfo>,
    pub capabilities: Option<ClientCapabilities>,
    pub initialized: bool,
}

impl DefaultMessageRouter {
    pub fn new() -> Self {
        Self {
            upstream_clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_upstream_client(&self, name: String, client: Arc<crate::mcp::McpClient>) {
        let mut clients = self.upstream_clients.write().await;
        clients.insert(name, client);
    }

    pub async fn remove_upstream_client(&self, name: &str) {
        let mut clients = self.upstream_clients.write().await;
        clients.remove(name);
    }

    pub async fn get_upstream_client(&self, name: &str) -> Option<Arc<crate::mcp::McpClient>> {
        let clients = self.upstream_clients.read().await;
        clients.get(name).cloned()
    }

    pub async fn list_upstream_clients(&self) -> Vec<String> {
        let clients = self.upstream_clients.read().await;
        clients.keys().cloned().collect()
    }
}

#[async_trait::async_trait]
impl MessageRouter for DefaultMessageRouter {
    async fn route_message(
        &self,
        connection_id: &str,
        message: McpMessage,
    ) -> Result<Option<McpMessage>> {
        debug!(
            "Routing message for connection {}: {:?}",
            connection_id, message
        );

        match message {
            McpMessage::Request(request) => {
                // Simple routing: try to find an upstream server that can handle this
                let clients = self.upstream_clients.read().await;

                // For now, just route to the first available client
                // In a real implementation, you'd have more sophisticated routing logic
                if let Some((_, client)) = clients.iter().next() {
                    if client.is_ready().await {
                        match client.send_request(&request.method, request.params).await {
                            Ok(response) => return Ok(Some(response)),
                            Err(e) => {
                                error!("Upstream request failed: {}", e);
                                return Ok(Some(McpMessage::new_error_response(
                                    request.id,
                                    McpError::server_error(&e.to_string()),
                                )));
                            }
                        }
                    }
                }

                // No upstream available
                Ok(Some(McpMessage::new_error_response(
                    request.id,
                    McpError::method_not_found(&request.method),
                )))
            }
            McpMessage::Response(_) => {
                // Responses are typically handled by the client that made the request
                Ok(None)
            }
            McpMessage::Notification(notification) => {
                // Broadcast notifications to all upstream servers
                let clients = self.upstream_clients.read().await;
                for (_, client) in clients.iter() {
                    if client.is_ready().await {
                        if let Err(e) = client
                            .send_notification(&notification.method, notification.params.clone())
                            .await
                        {
                            warn!("Failed to forward notification to upstream: {}", e);
                        }
                    }
                }
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_server_creation() {
        let router = Arc::new(DefaultMessageRouter::new());
        let server = McpServer::new("test-server".to_string(), router);

        assert_eq!(server.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_connection_info() {
        let router = Arc::new(DefaultMessageRouter::new());
        let server = McpServer::new("test-server".to_string(), router);

        let connections = server.list_connections().await;
        assert!(connections.is_empty());
    }

    #[tokio::test]
    async fn test_message_router() {
        let router = DefaultMessageRouter::new();

        let clients = router.list_upstream_clients().await;
        assert!(clients.is_empty());
    }
}
