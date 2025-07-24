use crate::config::Config;
use crate::error::{GatewayError, Result};
use axum::{extract::ConnectInfo, response::Response, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    catch_panic::CatchPanicLayer,
    normalize_path::NormalizePathLayer,
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    validate_request::ValidateRequestHeaderLayer,
};
use tracing::{error, info, warn};

/// HTTP server for the MCP Gateway
#[derive(Clone)]
pub struct HttpServer {
    bind_addr: SocketAddr,
    router: Router,
    config: Config,
}

impl HttpServer {
    pub fn new(bind_addr: SocketAddr, router: Router, config: Config) -> Self {
        Self {
            bind_addr,
            router,
            config,
        }
    }

    /// Start the HTTP server
    pub async fn start(self) -> Result<()> {
        info!("Starting HTTP server on {}", self.bind_addr);

        // Create TCP listener
        let listener = TcpListener::bind(self.bind_addr).await.map_err(|e| {
            GatewayError::Server(format!("Failed to bind to {}: {}", self.bind_addr, e))
        })?;

        info!("HTTP server listening on {}", self.bind_addr);

        // Add server-level middleware
        let router = self.router;
        let app = self.add_server_middleware(router);

        // Serve with graceful shutdown
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| GatewayError::Server(format!("Server error: {}", e)))?;

        info!("HTTP server stopped");
        Ok(())
    }

    /// Add server-level middleware layers
    fn add_server_middleware(self, router: Router) -> Router {
        let mut service_builder = ServiceBuilder::new();

        // Add panic recovery
        service_builder = service_builder.layer(CatchPanicLayer::custom(handle_panic));

        // Add path normalization
        service_builder = service_builder.layer(NormalizePathLayer::trim_trailing_slash());

        // Add security headers
        service_builder = service_builder.layer(SetResponseHeaderLayer::overriding(
            axum::http::header::SERVER,
            axum::http::HeaderValue::from_static("MCP-Gateway"),
        ));

        service_builder = service_builder.layer(SetResponseHeaderLayer::appending(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ));

        service_builder = service_builder.layer(SetResponseHeaderLayer::appending(
            axum::http::header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ));

        service_builder = service_builder.layer(SetResponseHeaderLayer::appending(
            axum::http::header::HeaderName::from_static("x-xss-protection"),
            axum::http::HeaderValue::from_static("1; mode=block"),
        ));

        service_builder = service_builder.layer(SetResponseHeaderLayer::appending(
            axum::http::header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ));

        // Add request validation if needed
        if self.config.server.max_connections.is_some() {
            // Add connection limiting middleware (would need custom implementation)
        }

        // Apply middleware stack to router
        router.layer(service_builder)
    }

    /// Create a static file server
    pub fn serve_static_files(path: &str, dir: &str) -> Router {
        Router::new().nest_service(path, ServeDir::new(dir))
    }

    /// Create a single file server
    pub fn serve_file(path: &str, file: &str) -> Router {
        Router::new().nest_service(path, ServeFile::new(file))
    }

    /// Get the bind address
    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    /// Update the router
    pub fn with_router(mut self, router: Router) -> Self {
        self.router = router;
        self
    }

    /// Add a route to the existing router
    pub fn add_route<T>(mut self, path: &str, method_router: axum::routing::MethodRouter<T>) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        self.router = self.router.route(path, method_router);
        self
    }

    /// Add a fallback handler
    pub fn with_fallback<H, T>(mut self, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.fallback(handler);
        self
    }

    /// Add state to the router
    pub fn with_state<S>(mut self, state: S) -> Self
    where
        S: Clone + Send + Sync + 'static,
    {
        self.router = self.router.with_state(state);
        self
    }
}

/// Handle panics in the server
fn handle_panic(err: Box<dyn std::any::Any + Send + 'static>) -> Response {
    let details = if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "Unknown panic occurred".to_string()
    };

    error!("Server panic: {}", details);

    axum::response::Response::builder()
        .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": "Internal server error",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }
            })
            .to_string(),
        )
        .unwrap()
        .into()
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Ctrl+C received, starting graceful shutdown");
        },
        _ = terminate => {
            info!("SIGTERM received, starting graceful shutdown");
        },
    }
}

/// Connection info extractor for logging client information
pub async fn log_connection_info(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    tracing::info!("Connection from: {}", addr);
    next.run(request).await
}

/// Health check handler
pub async fn health_check() -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    Ok(axum::Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "mcp-gateway",
        "version": env!("CARGO_PKG_VERSION")
    })))
}

/// Readiness check handler
pub async fn readiness_check() -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    Ok(axum::Json(serde_json::json!({
        "status": "ready",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "mcp-gateway",
        "version": env!("CARGO_PKG_VERSION")
    })))
}

/// Liveness check handler
pub async fn liveness_check() -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    Ok(axum::Json(serde_json::json!({
        "status": "alive",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "mcp-gateway",
        "version": env!("CARGO_PKG_VERSION")
    })))
}

/// Default 404 handler
pub async fn not_found() -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_FOUND
}

/// Default error handler
pub async fn handle_error(
    err: Box<dyn std::error::Error + Send + Sync>,
) -> axum::response::Response {
    error!("Unhandled error: {}", err);

    axum::response::Response::builder()
        .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": "An internal error occurred",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }
            })
            .to_string(),
        )
        .unwrap()
        .into()
}

/// Create a basic router with common endpoints
pub fn create_basic_router() -> Router {
    Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/readiness", axum::routing::get(readiness_check))
        .route("/liveness", axum::routing::get(liveness_check))
        .fallback(not_found)
}

/// Server configuration builder
pub struct ServerBuilder {
    bind_addr: Option<SocketAddr>,
    router: Option<Router>,
    config: Option<Config>,
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            bind_addr: None,
            router: None,
            config: None,
        }
    }

    pub fn bind_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = Some(addr);
        self
    }

    pub fn router(mut self, router: Router) -> Self {
        self.router = Some(router);
        self
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn build(self) -> Result<HttpServer> {
        let bind_addr = self
            .bind_addr
            .ok_or_else(|| GatewayError::Server("Bind address is required".to_string()))?;

        let router = self.router.unwrap_or_else(|| create_basic_router());

        let config = self.config.unwrap_or_default();

        Ok(HttpServer::new(bind_addr, router, config))
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_server_builder() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let config = Config::default();

        let server = ServerBuilder::new().bind_addr(addr).config(config).build();

        assert!(server.is_ok());
        let server = server.unwrap();
        assert_eq!(server.bind_addr(), addr);
    }

    #[test]
    fn test_server_builder_missing_addr() {
        let config = Config::default();

        let server = ServerBuilder::new().config(config).build();

        assert!(server.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert!(response.is_ok());

        let json = response.unwrap();
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["service"], "mcp-gateway");
    }

    #[tokio::test]
    async fn test_readiness_check() {
        let response = readiness_check().await;
        assert!(response.is_ok());

        let json = response.unwrap();
        assert_eq!(json["status"], "ready");
    }

    #[tokio::test]
    async fn test_liveness_check() {
        let response = liveness_check().await;
        assert!(response.is_ok());

        let json = response.unwrap();
        assert_eq!(json["status"], "alive");
    }

    #[test]
    fn test_basic_router_creation() {
        let router = create_basic_router();
        // Router creation should not panic
        assert!(true);
    }

    #[test]
    fn test_server_creation() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let router = create_basic_router();
        let config = Config::default();

        let server = HttpServer::new(addr, router, config);
        assert_eq!(server.bind_addr(), addr);
    }
}
