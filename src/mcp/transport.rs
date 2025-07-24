use crate::error::{GatewayError, Result};
use crate::mcp::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpStream, UnixStream};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;

/// Transport types supported by MCP
#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    WebSocket,
    Http,
    Stdio,
    Unix,
    Tcp,
}

/// MCP transport configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub transport_type: TransportType,
    pub url: Option<Url>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub working_directory: Option<std::path::PathBuf>,
    pub timeout: Duration,
    pub reconnect_attempts: u32,
    pub reconnect_delay: Duration,
}

/// Generic MCP transport trait
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn send_message(&mut self, message: McpMessage) -> Result<()>;
    async fn receive_message(&mut self) -> Result<Option<McpMessage>>;
    async fn is_connected(&self) -> bool;
    fn transport_type(&self) -> TransportType;
}

/// WebSocket transport implementation
pub struct WebSocketTransport {
    config: TransportConfig,
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    connected: bool,
}

impl WebSocketTransport {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            config,
            stream: None,
            connected: false,
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for WebSocketTransport {
    async fn connect(&mut self) -> Result<()> {
        if self.connected {
            return Ok(());
        }

        let url = self.config.url.as_ref().ok_or_else(|| {
            GatewayError::McpProtocol("URL required for WebSocket transport".to_string())
        })?;

        info!("Connecting to WebSocket: {}", url);

        let (ws_stream, _) = connect_async(url).await.map_err(|e| {
            GatewayError::McpProtocol(format!("WebSocket connection failed: {}", e))
        })?;

        self.stream = Some(ws_stream);
        self.connected = true;

        info!("WebSocket connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.close(None).await;
        }
        self.connected = false;
        info!("WebSocket disconnected");
        Ok(())
    }

    async fn send_message(&mut self, message: McpMessage) -> Result<()> {
        if !self.connected {
            return Err(GatewayError::McpProtocol("Not connected".to_string()));
        }

        let json = serde_json::to_string(&message)?;

        if let Some(ref mut stream) = self.stream {
            stream.send(Message::Text(json)).await.map_err(|e| {
                self.connected = false;
                GatewayError::WebSocket(e)
            })?;
        }

        Ok(())
    }

    async fn receive_message(&mut self) -> Result<Option<McpMessage>> {
        if !self.connected {
            return Err(GatewayError::McpProtocol("Not connected".to_string()));
        }

        if let Some(ref mut stream) = self.stream {
            match stream.next().await {
                Some(Ok(Message::Text(text))) => {
                    let message = serde_json::from_str(&text)?;
                    Ok(Some(message))
                }
                Some(Ok(Message::Close(_))) => {
                    self.connected = false;
                    Ok(None)
                }
                Some(Ok(_)) => Ok(None), // Ignore other message types
                Some(Err(e)) => {
                    self.connected = false;
                    Err(GatewayError::WebSocket(e))
                }
                None => {
                    self.connected = false;
                    Ok(None)
                }
            }
        } else {
            Err(GatewayError::McpProtocol(
                "Stream not available".to_string(),
            ))
        }
    }

    async fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> TransportType {
        TransportType::WebSocket
    }
}

/// HTTP transport implementation
pub struct HttpTransport {
    config: TransportConfig,
    client: reqwest::Client,
    connected: bool,
}

impl HttpTransport {
    pub fn new(config: TransportConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap();

        Self {
            config,
            client,
            connected: false,
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for HttpTransport {
    async fn connect(&mut self) -> Result<()> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    async fn send_message(&mut self, message: McpMessage) -> Result<()> {
        if !self.connected {
            return Err(GatewayError::McpProtocol("Not connected".to_string()));
        }

        let url = self.config.url.as_ref().ok_or_else(|| {
            GatewayError::McpProtocol("URL required for HTTP transport".to_string())
        })?;

        let json = serde_json::to_string(&message)?;

        let response = self
            .client
            .post(url.clone())
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(GatewayError::Http(reqwest::Error::from(
                response.error_for_status().unwrap_err(),
            )));
        }

        Ok(())
    }

    async fn receive_message(&mut self) -> Result<Option<McpMessage>> {
        // HTTP transport typically doesn't receive messages directly
        // This would be implemented using Server-Sent Events or polling
        Ok(None)
    }

    async fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Http
    }
}

/// Stdio transport implementation for subprocess communication
pub struct StdioTransport {
    config: TransportConfig,
    process: Option<Child>,
    stdin: Option<tokio::process::ChildStdin>,
    stdout_reader: Option<BufReader<tokio::process::ChildStdout>>,
    connected: bool,
}

impl StdioTransport {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            config,
            process: None,
            stdin: None,
            stdout_reader: None,
            connected: false,
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn connect(&mut self) -> Result<()> {
        if self.connected {
            return Ok(());
        }

        let command = self.config.command.as_ref().ok_or_else(|| {
            GatewayError::McpProtocol("Command required for stdio transport".to_string())
        })?;

        info!("Starting process: {}", command);

        let mut cmd = Command::new(command);

        if let Some(ref args) = self.config.args {
            cmd.args(args);
        }

        if let Some(ref env) = self.config.env {
            cmd.envs(env);
        }

        if let Some(ref working_dir) = self.config.working_directory {
            cmd.current_dir(working_dir);
        }

        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| GatewayError::McpProtocol(format!("Failed to start process: {}", e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| GatewayError::McpProtocol("Failed to get stdin handle".to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| GatewayError::McpProtocol("Failed to get stdout handle".to_string()))?;

        self.stdin = Some(stdin);
        self.stdout_reader = Some(BufReader::new(stdout));
        self.process = Some(child);
        self.connected = true;

        info!("Stdio transport connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
            let _ = process.wait().await;
        }

        self.stdin = None;
        self.stdout_reader = None;
        self.connected = false;

        info!("Stdio transport disconnected");
        Ok(())
    }

    async fn send_message(&mut self, message: McpMessage) -> Result<()> {
        if !self.connected {
            return Err(GatewayError::McpProtocol("Not connected".to_string()));
        }

        let json = serde_json::to_string(&message)?;
        let data = format!("{}\n", json);

        if let Some(ref mut stdin) = self.stdin {
            stdin.write_all(data.as_bytes()).await.map_err(|e| {
                self.connected = false;
                GatewayError::Io(e)
            })?;
            stdin.flush().await.map_err(|e| {
                self.connected = false;
                GatewayError::Io(e)
            })?;
        }

        Ok(())
    }

    async fn receive_message(&mut self) -> Result<Option<McpMessage>> {
        if !self.connected {
            return Err(GatewayError::McpProtocol("Not connected".to_string()));
        }

        if let Some(ref mut reader) = self.stdout_reader {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF
                    self.connected = false;
                    Ok(None)
                }
                Ok(_) => {
                    let line = line.trim();
                    if line.is_empty() {
                        return Ok(None);
                    }

                    let message = serde_json::from_str(line)?;
                    Ok(Some(message))
                }
                Err(e) => {
                    self.connected = false;
                    Err(GatewayError::Io(e))
                }
            }
        } else {
            Err(GatewayError::McpProtocol(
                "Reader not available".to_string(),
            ))
        }
    }

    async fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Stdio
    }
}

/// Unix socket transport implementation
pub struct UnixTransport {
    config: TransportConfig,
    stream: Option<UnixStream>,
    reader: Option<BufReader<tokio::io::ReadHalf<UnixStream>>>,
    writer: Option<tokio::io::WriteHalf<UnixStream>>,
    connected: bool,
}

impl UnixTransport {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            config,
            stream: None,
            reader: None,
            writer: None,
            connected: false,
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for UnixTransport {
    async fn connect(&mut self) -> Result<()> {
        if self.connected {
            return Ok(());
        }

        let path = self
            .config
            .url
            .as_ref()
            .and_then(|u| u.to_file_path().ok())
            .ok_or_else(|| {
                GatewayError::McpProtocol("Valid file path required for Unix transport".to_string())
            })?;

        info!("Connecting to Unix socket: {:?}", path);

        let stream = UnixStream::connect(&path).await.map_err(|e| {
            GatewayError::McpProtocol(format!("Unix socket connection failed: {}", e))
        })?;

        let (read_half, write_half) = tokio::io::split(stream);
        self.reader = Some(BufReader::new(read_half));
        self.writer = Some(write_half);
        self.connected = true;

        info!("Unix socket connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.reader = None;
        self.writer = None;
        self.stream = None;
        self.connected = false;
        info!("Unix socket disconnected");
        Ok(())
    }

    async fn send_message(&mut self, message: McpMessage) -> Result<()> {
        if !self.connected {
            return Err(GatewayError::McpProtocol("Not connected".to_string()));
        }

        let json = serde_json::to_string(&message)?;
        let data = format!("{}\n", json);

        if let Some(ref mut writer) = self.writer {
            writer.write_all(data.as_bytes()).await.map_err(|e| {
                self.connected = false;
                GatewayError::Io(e)
            })?;
            writer.flush().await.map_err(|e| {
                self.connected = false;
                GatewayError::Io(e)
            })?;
        }

        Ok(())
    }

    async fn receive_message(&mut self) -> Result<Option<McpMessage>> {
        if !self.connected {
            return Err(GatewayError::McpProtocol("Not connected".to_string()));
        }

        if let Some(ref mut reader) = self.reader {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    self.connected = false;
                    Ok(None)
                }
                Ok(_) => {
                    let line = line.trim();
                    if line.is_empty() {
                        return Ok(None);
                    }

                    let message = serde_json::from_str(line)?;
                    Ok(Some(message))
                }
                Err(e) => {
                    self.connected = false;
                    Err(GatewayError::Io(e))
                }
            }
        } else {
            Err(GatewayError::McpProtocol(
                "Reader not available".to_string(),
            ))
        }
    }

    async fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Unix
    }
}

/// Transport factory for creating transport instances
pub struct TransportFactory;

impl TransportFactory {
    pub fn create_transport(config: TransportConfig) -> Box<dyn McpTransport> {
        match config.transport_type {
            TransportType::WebSocket => Box::new(WebSocketTransport::new(config)),
            TransportType::Http => Box::new(HttpTransport::new(config)),
            TransportType::Stdio => Box::new(StdioTransport::new(config)),
            TransportType::Unix => Box::new(UnixTransport::new(config)),
            TransportType::Tcp => {
                // For TCP, we'll use WebSocket over TCP
                Box::new(WebSocketTransport::new(config))
            }
        }
    }

    pub fn from_url(url: &str) -> Result<Box<dyn McpTransport>> {
        let parsed_url = Url::parse(url)
            .map_err(|e| GatewayError::McpProtocol(format!("Invalid URL: {}", e)))?;

        let transport_type = match parsed_url.scheme() {
            "ws" | "wss" => TransportType::WebSocket,
            "http" | "https" => TransportType::Http,
            "unix" => TransportType::Unix,
            _ => {
                return Err(GatewayError::McpProtocol(format!(
                    "Unsupported URL scheme: {}",
                    parsed_url.scheme()
                )))
            }
        };

        let config = TransportConfig {
            transport_type,
            url: Some(parsed_url),
            command: None,
            args: None,
            env: None,
            working_directory: None,
            timeout: Duration::from_secs(30),
            reconnect_attempts: 3,
            reconnect_delay: Duration::from_secs(5),
        };

        Ok(Self::create_transport(config))
    }

    pub fn from_command(command: &str, args: Option<Vec<String>>) -> Box<dyn McpTransport> {
        let config = TransportConfig {
            transport_type: TransportType::Stdio,
            url: None,
            command: Some(command.to_string()),
            args,
            env: None,
            working_directory: None,
            timeout: Duration::from_secs(30),
            reconnect_attempts: 3,
            reconnect_delay: Duration::from_secs(5),
        };

        Self::create_transport(config)
    }
}

/// Transport manager for handling multiple transports with reconnection
pub struct TransportManager {
    transports: Arc<RwLock<std::collections::HashMap<String, Box<dyn McpTransport>>>>,
    reconnect_tasks: Arc<RwLock<std::collections::HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl TransportManager {
    pub fn new() -> Self {
        Self {
            transports: Arc::new(RwLock::new(std::collections::HashMap::new())),
            reconnect_tasks: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn add_transport(&self, name: String, mut transport: Box<dyn McpTransport>) {
        // Connect the transport
        if let Err(e) = transport.connect().await {
            error!("Failed to connect transport {}: {}", name, e);
        }

        let mut transports = self.transports.write().await;
        transports.insert(name, transport);
    }

    pub async fn remove_transport(&self, name: &str) -> Result<()> {
        let mut transports = self.transports.write().await;
        if let Some(mut transport) = transports.remove(name) {
            transport.disconnect().await?;
        }

        let mut tasks = self.reconnect_tasks.write().await;
        if let Some(task) = tasks.remove(name) {
            task.abort();
        }

        Ok(())
    }

    pub async fn send_message(&self, transport_name: &str, message: McpMessage) -> Result<()> {
        let mut transports = self.transports.write().await;
        if let Some(transport) = transports.get_mut(transport_name) {
            transport.send_message(message).await
        } else {
            Err(GatewayError::NotFound(format!(
                "Transport {}",
                transport_name
            )))
        }
    }

    pub async fn receive_message(&self, transport_name: &str) -> Result<Option<McpMessage>> {
        let mut transports = self.transports.write().await;
        if let Some(transport) = transports.get_mut(transport_name) {
            transport.receive_message().await
        } else {
            Err(GatewayError::NotFound(format!(
                "Transport {}",
                transport_name
            )))
        }
    }

    pub async fn is_connected(&self, transport_name: &str) -> bool {
        let transports = self.transports.read().await;
        if let Some(transport) = transports.get(transport_name) {
            transport.is_connected().await
        } else {
            false
        }
    }

    pub async fn list_transports(&self) -> Vec<String> {
        let transports = self.transports.read().await;
        transports.keys().cloned().collect()
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            transport_type: TransportType::WebSocket,
            url: None,
            command: None,
            args: None,
            env: None,
            working_directory: None,
            timeout: Duration::from_secs(30),
            reconnect_attempts: 3,
            reconnect_delay: Duration::from_secs(5),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_factory_from_url() {
        let transport = TransportFactory::from_url("ws://localhost:8080").unwrap();
        assert_eq!(transport.transport_type(), TransportType::WebSocket);

        let transport = TransportFactory::from_url("http://localhost:8080").unwrap();
        assert_eq!(transport.transport_type(), TransportType::Http);

        assert!(TransportFactory::from_url("invalid://url").is_err());
    }

    #[test]
    fn test_transport_factory_from_command() {
        let transport = TransportFactory::from_command("echo", Some(vec!["hello".to_string()]));
        assert_eq!(transport.transport_type(), TransportType::Stdio);
    }

    #[tokio::test]
    async fn test_transport_manager() {
        let manager = TransportManager::new();

        let transport = TransportFactory::from_url("ws://localhost:8080").unwrap();
        manager.add_transport("test".to_string(), transport).await;

        let transports = manager.list_transports().await;
        assert!(transports.contains(&"test".to_string()));

        manager.remove_transport("test").await.unwrap();
        let transports = manager.list_transports().await;
        assert!(!transports.contains(&"test".to_string()));
    }
}
