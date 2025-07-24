use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Server error: {0}")]
    Server(String),

    #[error("MCP protocol error: {0}")]
    McpProtocol(String),

    #[error("Upstream server error: {server_id}: {message}")]
    UpstreamServer { server_id: String, message: String },

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Authorization error: {0}")]
    Authorization(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Circuit breaker is open for server: {0}")]
    CircuitBreakerOpen(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Health check failed for server: {0}")]
    HealthCheckFailed(String),

    #[error("Load balancer error: {0}")]
    LoadBalancer(String),

    #[error("Middleware error: {0}")]
    Middleware(String),
}

impl GatewayError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            GatewayError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::Server(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::McpProtocol(_) => StatusCode::BAD_REQUEST,
            GatewayError::UpstreamServer { .. } => StatusCode::BAD_GATEWAY,
            GatewayError::Authentication(_) => StatusCode::UNAUTHORIZED,
            GatewayError::Authorization(_) => StatusCode::FORBIDDEN,
            GatewayError::RateLimit => StatusCode::TOO_MANY_REQUESTS,
            GatewayError::CircuitBreakerOpen(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
            GatewayError::Serialization(_) => StatusCode::BAD_REQUEST,
            GatewayError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            GatewayError::NotFound(_) => StatusCode::NOT_FOUND,
            GatewayError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::Validation(_) => StatusCode::BAD_REQUEST,
            GatewayError::HealthCheckFailed(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::LoadBalancer(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::Middleware(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            GatewayError::Config(_) => "CONFIG_ERROR",
            GatewayError::Server(_) => "SERVER_ERROR",
            GatewayError::McpProtocol(_) => "MCP_PROTOCOL_ERROR",
            GatewayError::UpstreamServer { .. } => "UPSTREAM_SERVER_ERROR",
            GatewayError::Authentication(_) => "AUTHENTICATION_ERROR",
            GatewayError::Authorization(_) => "AUTHORIZATION_ERROR",
            GatewayError::RateLimit => "RATE_LIMIT_EXCEEDED",
            GatewayError::CircuitBreakerOpen(_) => "CIRCUIT_BREAKER_OPEN",
            GatewayError::Timeout(_) => "TIMEOUT_ERROR",
            GatewayError::Serialization(_) => "SERIALIZATION_ERROR",
            GatewayError::Io(_) => "IO_ERROR",
            GatewayError::InvalidRequest(_) => "INVALID_REQUEST",
            GatewayError::NotFound(_) => "NOT_FOUND",
            GatewayError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            GatewayError::Internal(_) => "INTERNAL_ERROR",
            GatewayError::Validation(_) => "VALIDATION_ERROR",
            GatewayError::HealthCheckFailed(_) => "HEALTH_CHECK_FAILED",
            GatewayError::LoadBalancer(_) => "LOAD_BALANCER_ERROR",
            GatewayError::Middleware(_) => "MIDDLEWARE_ERROR",
        }
    }
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        tracing::error!("Gateway error: {}", self);

        let body = Json(json!({
            "error": {
                "code": self.error_code(),
                "message": self.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }
        }));

        (self.status_code(), body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, GatewayError>;

// Helper macro for creating internal errors with context
#[macro_export]
macro_rules! internal_error {
    ($msg:expr) => {
        $crate::error::GatewayError::Internal($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::GatewayError::Internal(format!($fmt, $($arg)*))
    };
}

// Helper macro for creating validation errors
#[macro_export]
macro_rules! validation_error {
    ($msg:expr) => {
        $crate::error::GatewayError::Validation($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::GatewayError::Validation(format!($fmt, $($arg)*))
    };
}

// Helper macro for creating upstream server errors
#[macro_export]
macro_rules! upstream_error {
    ($server_id:expr, $msg:expr) => {
        $crate::error::GatewayError::UpstreamServer {
            server_id: $server_id.to_string(),
            message: $msg.to_string(),
        }
    };
    ($server_id:expr, $fmt:expr, $($arg:tt)*) => {
        $crate::error::GatewayError::UpstreamServer {
            server_id: $server_id.to_string(),
            message: format!($fmt, $($arg)*),
        }
    };
}
