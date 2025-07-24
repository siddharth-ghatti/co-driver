use axum::{extract::Request, middleware::Next, response::Response};

/// Logging middleware (placeholder - uses tower-http TraceLayer in mod.rs)
pub struct LoggingMiddleware;

impl LoggingMiddleware {
    pub fn new() -> Self {
        Self
    }
}

pub async fn logging_middleware(request: Request, next: Next) -> Response {
    // This is handled by tower-http TraceLayer in the main middleware stack
    next.run(request).await
}
