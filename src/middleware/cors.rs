use axum::{extract::Request, middleware::Next, response::Response};

/// CORS middleware (placeholder - uses tower-http CorsLayer in mod.rs)
pub struct CorsMiddleware;

impl CorsMiddleware {
    pub fn new() -> Self {
        Self
    }
}

pub async fn cors_middleware(request: Request, next: Next) -> Response {
    // This is handled by tower-http CorsLayer in the main middleware stack
    next.run(request).await
}
