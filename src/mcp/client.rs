use crate::error::{GatewayError, Result};
use crate::mcp::*;
use reqwest::Client as HttpClient;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;

/// Connection state for an MCP client
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Initializing,
    Ready,
    Error(String),
}

/// MCP client for connecting to upstream servers
pub struct McpClient {
    server_id: String,
    server_url: Url,
    connection_state: Arc<RwLock<ConnectionState>>,
    protocol: Arc<RwLock<McpProtocol>>,
    pending_requests: Arc<RwLock<HashMap<Value, tokio::sync::oneshot::Sender<McpMessage>>>>,
    http_client: HttpClient,
    websocket_stream: Arc<
        RwLock<Option<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>>,
    >,
    request_timeout: Duration,
    ping_interval: Duration,
    last_activity: Arc<RwLock<Instant>>,
    capabilities: Arc<RwLock<Option<ServerCapabilities>>>,
    server_info: Arc<RwLock<Option<ServerInfo>>>,
}

impl McpClient {
    pub fn new(server_id: String, server_url: Url) -> Self {
        Self {
            server_id,
            server_url,
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            protocol: Arc::new(RwLock::new(McpProtocol::new(
                "mcp-gateway".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            ))),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            http_client: HttpClient::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            websocket_stream: Arc::new(RwLock::new(None)),
            request_timeout: Duration::from_secs(30),
            ping_interval: Duration::from_secs(30),
            last_activity: Arc::new(RwLock::new(Instant::now())),
            capabilities: Arc::new(RwLock::new(None)),
            server_info: Arc::new(RwLock::new(None)),
        }
    }

    /// Connect to the MCP server
    pub async fn connect(&self) -> Result<()> {
        let mut state = self.connection_state.write().await;
        if *state == ConnectionState::Connected || *state == ConnectionState::Ready {
            return Ok(());
        }

        *state = ConnectionState::Connecting;
        drop(state);

        info!("Connecting to MCP server: {}", self.server_url);

        match self.server_url.scheme() {
            "ws" | "wss" => self.connect_websocket().await,
            "http" | "https" => self.connect_http().await,
            _ => Err(GatewayError::McpProtocol(format!(
                "Unsupported URL scheme: {}",
                self.server_url.scheme()
            ))),
        }
    }

    async fn connect_websocket(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.server_url).await.map_err(|e| {
            GatewayError::McpProtocol(format!("WebSocket connection failed: {}", e))
        })?;

        {
            let mut stream = self.websocket_stream.write().await;
            *stream = Some(ws_stream);
        }

        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connected;
        }

        self.initialize_connection().await?;
        self.start_message_handler().await;

        Ok(())
    }

    async fn connect_http(&self) -> Result<()> {
        // For HTTP-based MCP connections, we'll use SSE or long polling
        // This is a simplified implementation
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connected;
        }

        self.initialize_connection().await?;

        Ok(())
    }

    async fn initialize_connection(&self) -> Result<()> {
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Initializing;
        }

        let init_params = InitializeParams {
            protocol_version: MCP_VERSION.to_string(),
            capabilities: ClientCapabilities {
                experimental: None,
                sampling: None,
            },
            client_info: ClientInfo {
                name: "mcp-gateway".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let response = self
            .send_request(
                methods::INITIALIZE,
                Some(serde_json::to_value(init_params)?),
            )
            .await?;

        if let McpMessage::Response(resp) = response {
            if let Some(error) = resp.error {
                return Err(GatewayError::McpProtocol(format!(
                    "Initialize failed: {}",
                    error.message
                )));
            }

            if let Some(result) = resp.result {
                let init_result: InitializeResult = serde_json::from_value(result)?;

                {
                    let mut capabilities = self.capabilities.write().await;
                    *capabilities = Some(init_result.capabilities);
                }

                {
                    let mut server_info = self.server_info.write().await;
                    *server_info = Some(init_result.server_info);
                }
            }
        }

        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Ready;
        }

        info!("MCP connection initialized for server: {}", self.server_id);
        Ok(())
    }

    async fn start_message_handler(&self) {
        let ws_stream = self.websocket_stream.clone();
        let pending_requests = self.pending_requests.clone();
        let connection_state = self.connection_state.clone();
        let last_activity = self.last_activity.clone();

        tokio::spawn(async move {
            let mut stream_guard = ws_stream.write().await;
            if let Some(ref mut stream) = *stream_guard {
                loop {
                    use futures_util::{SinkExt, StreamExt};

                    match stream.next().await {
                        Some(Ok(message)) => {
                            {
                                let mut activity = last_activity.write().await;
                                *activity = Instant::now();
                            }

                            if let Ok(text) = message.to_text() {
                                if let Ok(mcp_message) = serde_json::from_str::<McpMessage>(text) {
                                    match mcp_message {
                                        McpMessage::Response(ref response) => {
                                            let mut pending = pending_requests.write().await;
                                            if let Some(sender) = pending.remove(&response.id) {
                                                let _ = sender.send(mcp_message);
                                            }
                                        }
                                        McpMessage::Notification(_) => {
                                            // Handle notifications (e.g., tools/list_changed)
                                            debug!("Received notification: {:?}", mcp_message);
                                        }
                                        _ => {
                                            warn!("Unexpected message type: {:?}", mcp_message);
                                        }
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            let mut state = connection_state.write().await;
                            *state = ConnectionState::Error(e.to_string());
                            break;
                        }
                        None => {
                            warn!("WebSocket connection closed");
                            let mut state = connection_state.write().await;
                            *state = ConnectionState::Disconnected;
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Send a request to the MCP server
    pub async fn send_request(&self, method: &str, params: Option<Value>) -> Result<McpMessage> {
        let state = self.connection_state.read().await;
        if *state != ConnectionState::Ready && *state != ConnectionState::Connected {
            return Err(GatewayError::McpProtocol(format!(
                "Client not ready: {:?}",
                *state
            )));
        }
        drop(state);

        let id = generate_request_id();
        let request = McpMessage::new_request(id.clone(), method.to_string(), params);

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(id, tx);
        }

        self.send_message(request).await?;

        match timeout(self.request_timeout, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(GatewayError::McpProtocol(
                "Request response channel closed".to_string(),
            )),
            Err(_) => Err(GatewayError::Timeout(format!(
                "Request timeout after {:?}",
                self.request_timeout
            ))),
        }
    }

    /// Send a notification to the MCP server
    pub async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        let notification = McpMessage::new_notification(method.to_string(), params);
        self.send_message(notification).await
    }

    async fn send_message(&self, message: McpMessage) -> Result<()> {
        let json = serde_json::to_string(&message)?;

        match self.server_url.scheme() {
            "ws" | "wss" => self.send_websocket_message(json).await,
            "http" | "https" => self.send_http_message(json).await,
            _ => Err(GatewayError::McpProtocol(format!(
                "Unsupported URL scheme: {}",
                self.server_url.scheme()
            ))),
        }
    }

    async fn send_websocket_message(&self, message: String) -> Result<()> {
        use futures_util::SinkExt;

        let mut stream_guard = self.websocket_stream.write().await;
        if let Some(ref mut stream) = *stream_guard {
            stream
                .send(Message::Text(message))
                .await
                .map_err(|e| GatewayError::WebSocket(e))?;
            Ok(())
        } else {
            Err(GatewayError::McpProtocol(
                "WebSocket not connected".to_string(),
            ))
        }
    }

    async fn send_http_message(&self, message: String) -> Result<()> {
        let response = self
            .http_client
            .post(self.server_url.clone())
            .header("Content-Type", "application/json")
            .body(message)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(GatewayError::Http(reqwest::Error::from(
                response.error_for_status().unwrap_err(),
            )));
        }

        Ok(())
    }

    /// Disconnect from the MCP server
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting from MCP server: {}", self.server_id);

        // Send shutdown notification
        if let Err(e) = self.send_notification(methods::SHUTDOWN, None).await {
            warn!("Failed to send shutdown notification: {}", e);
        }

        {
            let mut stream = self.websocket_stream.write().await;
            if let Some(ws_stream) = stream.take() {
                drop(ws_stream);
            }
        }

        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Disconnected;
        }

        {
            let mut pending = self.pending_requests.write().await;
            pending.clear();
        }

        Ok(())
    }

    /// Get the current connection state
    pub async fn connection_state(&self) -> ConnectionState {
        self.connection_state.read().await.clone()
    }

    /// Check if the client is ready to handle requests
    pub async fn is_ready(&self) -> bool {
        let state = self.connection_state.read().await;
        *state == ConnectionState::Ready
    }

    /// Get server capabilities
    pub async fn capabilities(&self) -> Option<ServerCapabilities> {
        self.capabilities.read().await.clone()
    }

    /// Get server info
    pub async fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }

    /// Health check ping
    pub async fn ping(&self) -> Result<Duration> {
        let start = Instant::now();
        self.send_request(methods::PING, None).await?;
        Ok(start.elapsed())
    }

    /// Get the server ID
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Get the server URL
    pub fn server_url(&self) -> &Url {
        &self.server_url
    }

    /// List available tools
    pub async fn list_tools(&self, cursor: Option<String>) -> Result<ToolsListResult> {
        let params = if cursor.is_some() {
            Some(serde_json::to_value(ToolsListParams { cursor })?)
        } else {
            None
        };

        let response = self.send_request(methods::TOOLS_LIST, params).await?;

        if let McpMessage::Response(resp) = response {
            if let Some(error) = resp.error {
                return Err(GatewayError::McpProtocol(error.message));
            }

            if let Some(result) = resp.result {
                let tools_result: ToolsListResult = serde_json::from_value(result)?;
                return Ok(tools_result);
            }
        }

        Err(GatewayError::McpProtocol(
            "Invalid response format".to_string(),
        ))
    }

    /// Call a tool
    pub async fn call_tool(
        &self,
        name: String,
        arguments: Option<Value>,
    ) -> Result<ToolCallResult> {
        let params = ToolCallParams { name, arguments };
        let response = self
            .send_request(methods::TOOLS_CALL, Some(serde_json::to_value(params)?))
            .await?;

        if let McpMessage::Response(resp) = response {
            if let Some(error) = resp.error {
                return Err(GatewayError::McpProtocol(error.message));
            }

            if let Some(result) = resp.result {
                let call_result: ToolCallResult = serde_json::from_value(result)?;
                return Ok(call_result);
            }
        }

        Err(GatewayError::McpProtocol(
            "Invalid response format".to_string(),
        ))
    }

    /// List available resources
    pub async fn list_resources(&self, cursor: Option<String>) -> Result<ResourcesListResult> {
        let params = if cursor.is_some() {
            Some(serde_json::to_value(ResourcesListParams { cursor })?)
        } else {
            None
        };

        let response = self.send_request(methods::RESOURCES_LIST, params).await?;

        if let McpMessage::Response(resp) = response {
            if let Some(error) = resp.error {
                return Err(GatewayError::McpProtocol(error.message));
            }

            if let Some(result) = resp.result {
                let resources_result: ResourcesListResult = serde_json::from_value(result)?;
                return Ok(resources_result);
            }
        }

        Err(GatewayError::McpProtocol(
            "Invalid response format".to_string(),
        ))
    }

    /// Read a resource
    pub async fn read_resource(&self, uri: String) -> Result<ResourcesReadResult> {
        let params = ResourcesReadParams { uri };
        let response = self
            .send_request(methods::RESOURCES_READ, Some(serde_json::to_value(params)?))
            .await?;

        if let McpMessage::Response(resp) = response {
            if let Some(error) = resp.error {
                return Err(GatewayError::McpProtocol(error.message));
            }

            if let Some(result) = resp.result {
                let read_result: ResourcesReadResult = serde_json::from_value(result)?;
                return Ok(read_result);
            }
        }

        Err(GatewayError::McpProtocol(
            "Invalid response format".to_string(),
        ))
    }

    /// List available prompts
    pub async fn list_prompts(&self, cursor: Option<String>) -> Result<PromptsListResult> {
        let params = if cursor.is_some() {
            Some(serde_json::to_value(PromptsListParams { cursor })?)
        } else {
            None
        };

        let response = self.send_request(methods::PROMPTS_LIST, params).await?;

        if let McpMessage::Response(resp) = response {
            if let Some(error) = resp.error {
                return Err(GatewayError::McpProtocol(error.message));
            }

            if let Some(result) = resp.result {
                let prompts_result: PromptsListResult = serde_json::from_value(result)?;
                return Ok(prompts_result);
            }
        }

        Err(GatewayError::McpProtocol(
            "Invalid response format".to_string(),
        ))
    }

    /// Get a prompt
    pub async fn get_prompt(
        &self,
        name: String,
        arguments: Option<HashMap<String, String>>,
    ) -> Result<PromptsGetResult> {
        let params = PromptsGetParams { name, arguments };
        let response = self
            .send_request(methods::PROMPTS_GET, Some(serde_json::to_value(params)?))
            .await?;

        if let McpMessage::Response(resp) = response {
            if let Some(error) = resp.error {
                return Err(GatewayError::McpProtocol(error.message));
            }

            if let Some(result) = resp.result {
                let get_result: PromptsGetResult = serde_json::from_value(result)?;
                return Ok(get_result);
            }
        }

        Err(GatewayError::McpProtocol(
            "Invalid response format".to_string(),
        ))
    }

    /// Start keepalive ping task
    pub async fn start_keepalive(&self) {
        let client = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(client.ping_interval);
            loop {
                interval.tick().await;

                if client.is_ready().await {
                    match client.ping().await {
                        Ok(latency) => {
                            debug!("Ping to {} successful: {:?}", client.server_id, latency);
                        }
                        Err(e) => {
                            warn!("Ping to {} failed: {}", client.server_id, e);
                            // Optionally implement reconnection logic here
                        }
                    }
                }
            }
        });
    }
}

impl Clone for McpClient {
    fn clone(&self) -> Self {
        Self {
            server_id: self.server_id.clone(),
            server_url: self.server_url.clone(),
            connection_state: self.connection_state.clone(),
            protocol: self.protocol.clone(),
            pending_requests: self.pending_requests.clone(),
            http_client: self.http_client.clone(),
            websocket_stream: self.websocket_stream.clone(),
            request_timeout: self.request_timeout,
            ping_interval: self.ping_interval,
            last_activity: self.last_activity.clone(),
            capabilities: self.capabilities.clone(),
            server_info: self.server_info.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let url = Url::parse("ws://localhost:8080").unwrap();
        let client = McpClient::new("test-server".to_string(), url);

        assert_eq!(client.server_id(), "test-server");
        assert!(!client.is_ready().await);
        assert_eq!(
            client.connection_state().await,
            ConnectionState::Disconnected
        );
    }

    #[tokio::test]
    async fn test_client_state_transitions() {
        let url = Url::parse("ws://localhost:8080").unwrap();
        let client = McpClient::new("test-server".to_string(), url);

        // Initial state should be disconnected
        assert_eq!(
            client.connection_state().await,
            ConnectionState::Disconnected
        );

        // Test state change
        {
            let mut state = client.connection_state.write().await;
            *state = ConnectionState::Connecting;
        }

        assert_eq!(client.connection_state().await, ConnectionState::Connecting);
    }
}
