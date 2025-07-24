use axum::{extract::Request, middleware::Next, response::Response};

/// Timeout middleware (placeholder - uses tower-http TimeoutLayer in mod.rs)
pub struct TimeoutMiddleware;

impl TimeoutMiddleware {
    pub fn new() -> Self {
        Self
    }
}

pub async fn timeout_middleware(request: Request, next: Next) -> Response {
    // This is handled by tower-http TimeoutLayer in the main middleware stack
    next.run(request).await
}
