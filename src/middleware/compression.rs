use axum::{extract::Request, middleware::Next, response::Response};

/// Compression middleware (placeholder - uses tower-http CompressionLayer in mod.rs)
pub struct CompressionMiddleware;

impl CompressionMiddleware {
    pub fn new() -> Self {
        Self
    }
}

pub async fn compression_middleware(request: Request, next: Next) -> Response {
    // This is handled by tower-http CompressionLayer in the main middleware stack
    next.run(request).await
}
