use crate::config::Config;
use crate::error::{GatewayError, Result};
use axum::{extract::WebSocketUpgrade, response::Response, routing::get, Json, Router};
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info};

/// Main Gateway struct that orchestrates all components
pub struct Gateway {
    config: Config,
    bind_addr: SocketAddr,
    metrics_addr: SocketAddr,
}

impl Gateway {
    /// Create a new Gateway instance
    pub async fn new(
        config: Config,
        bind_addr: SocketAddr,
        metrics_addr: SocketAddr,
    ) -> Result<Self> {
        info!("Initializing MCP Gateway");

        Ok(Self {
            config,
            bind_addr,
            metrics_addr,
        })
    }

    /// Start the gateway server
    pub async fn start(self) -> Result<()> {
        info!("Starting MCP Gateway on {}", self.bind_addr);

        // Create the router
        let app = self.create_router();

        // Create TCP listener
        let listener = TcpListener::bind(self.bind_addr).await.map_err(|e| {
            GatewayError::Server(format!("Failed to bind to {}: {}", self.bind_addr, e))
        })?;

        info!("HTTP server listening on {}", self.bind_addr);

        // Start the server
        axum::serve(listener, app)
            .await
            .map_err(|e| GatewayError::Server(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Start the metrics server
    pub async fn start_metrics_server(&self) -> Result<()> {
        if !self.config.metrics.enabled {
            return Ok(());
        }

        info!("Starting metrics server on {}", self.metrics_addr);

        let metrics_router = Router::new().route(
            &self.config.metrics.path,
            get(|| async {
                "# HELP mcp_gateway_info Gateway information\n\
                 # TYPE mcp_gateway_info gauge\n\
                 mcp_gateway_info{version=\"0.1.0\"} 1\n"
            }),
        );

        let listener = TcpListener::bind(self.metrics_addr).await.map_err(|e| {
            GatewayError::Server(format!(
                "Failed to bind metrics server to {}: {}",
                self.metrics_addr, e
            ))
        })?;

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, metrics_router).await {
                error!("Metrics server error: {}", e);
            }
        });

        Ok(())
    }

    /// Create the main HTTP router
    fn create_router(&self) -> Router {
        Router::new()
            // MCP WebSocket endpoint
            .route("/mcp", get(handle_websocket))
            // Health check endpoints
            .route("/health", get(health_check))
            .route("/readiness", get(readiness_check))
            .route("/liveness", get(liveness_check))
            // Status endpoint
            .route("/status", get(status_check))
            // Management API
            .route("/api/v1/connections", get(list_connections))
    }
}

/// Handle WebSocket upgrade for MCP connections
async fn handle_websocket(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_mcp_connection)
}

/// Handle an MCP WebSocket connection
async fn handle_mcp_connection(socket: axum::extract::ws::WebSocket) {
    info!("New MCP WebSocket connection established");

    // This is a simplified implementation
    // In a real gateway, this would handle the MCP protocol
    let (mut sender, mut receiver) = socket.split();

    // Simple echo server for demonstration
    while let Some(msg) = receiver.next().await {
        if let Ok(msg) = msg {
            if let Ok(text) = msg.to_text() {
                info!("Received MCP message: {:?}", text);

                // Echo back a simple response
                let response = axum::extract::ws::Message::Text(
                    r#"{"jsonrpc":"2.0","id":1,"result":{"status":"ok"}}"#.to_string(),
                );

                if sender.send(response).await.is_err() {
                    break;
                }
            }
        } else {
            break;
        }
    }

    info!("MCP WebSocket connection closed");
}

/// Health check handler
async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "mcp-gateway",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Readiness check handler
async fn readiness_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ready",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "mcp-gateway",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Liveness check handler
async fn liveness_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "alive",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "mcp-gateway",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Status check handler
async fn status_check() -> Json<serde_json::Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "upstream_servers": 0,
        "healthy_upstreams": 0,
        "active_connections": 0,
        "metrics_enabled": true,
        "health_check_enabled": true
    }))
}

/// List connections handler
async fn list_connections() -> Json<serde_json::Value> {
    Json(json!({
        "connections": []
    }))
}

// Required import for WebSocket handling
use futures_util::{SinkExt, StreamExt};
