use axum::{extract::Request, middleware::Next, response::Response};

/// Request ID middleware (placeholder - uses tower-http RequestIdLayer in mod.rs)
pub struct RequestIdMiddleware;

impl RequestIdMiddleware {
    pub fn new() -> Self {
        Self
    }
}

pub async fn request_id_middleware(request: Request, next: Next) -> Response {
    // This is handled by tower-http SetRequestIdLayer and PropagateRequestIdLayer in the main middleware stack
    next.run(request).await
}
