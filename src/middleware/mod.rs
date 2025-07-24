pub mod auth;
pub mod compression;
pub mod cors;
pub mod logging;
pub mod rate_limit;
pub mod request_id;
pub mod timeout;
pub mod transform;

use crate::config::MiddlewareConfig;
use axum::{extract::Request, middleware::Next, response::Response, Router};
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::Level;
use uuid::Uuid;

pub use auth::AuthMiddleware;
pub use compression::CompressionMiddleware;
pub use cors::CorsMiddleware;
pub use logging::LoggingMiddleware;
pub use rate_limit::RateLimitMiddleware;
pub use request_id::RequestIdMiddleware;
pub use timeout::TimeoutMiddleware;
pub use transform::TransformMiddleware;

/// Middleware stack that applies all configured middleware layers
pub struct MiddlewareStack {
    config: MiddlewareConfig,
}

impl MiddlewareStack {
    pub fn new(config: &MiddlewareConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Apply all middleware layers to the router
    pub fn apply(&self, router: Router) -> Router {
        let mut service_builder = ServiceBuilder::new();

        // Request ID middleware (should be first)
        if self.config.request_id.enabled {
            service_builder = service_builder
                .layer(SetRequestIdLayer::x_request_id(McpRequestIdMaker))
                .layer(PropagateRequestIdLayer::x_request_id());
        }

        // Timeout middleware
        if let Some(timeout_secs) = self.config.timeout.request {
            service_builder =
                service_builder.layer(TimeoutLayer::new(Duration::from_secs(timeout_secs)));
        }

        // Compression middleware
        if self.config.compression.enabled {
            service_builder = service_builder.layer(CompressionLayer::new());
        }

        // Tracing middleware
        service_builder = service_builder.layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request| {
                    let request_id = request
                        .headers()
                        .get("x-request-id")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("unknown");

                    tracing::info_span!(
                        "request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = %request_id,
                    )
                })
                .on_request(|_request: &Request, _span: &tracing::Span| {
                    tracing::info!("started processing request")
                })
                .on_response(
                    |response: &Response, latency: Duration, _span: &tracing::Span| {
                        tracing::info!(
                            status = %response.status(),
                            latency = ?latency,
                            "finished processing request"
                        )
                    },
                )
                .on_failure(
                    |error: tower_http::classify::ServerErrorsFailureClass,
                     latency: Duration,
                     _span: &tracing::Span| {
                        tracing::error!(
                            error = %error,
                            latency = ?latency,
                            "request failed"
                        )
                    },
                ),
        );

        // Apply the service builder to the router
        router.layer(service_builder)
    }

    /// Apply authentication middleware
    pub fn with_auth(self, router: Router) -> Router {
        if self.config.request_id.enabled {
            router.layer(axum::middleware::from_fn(auth_middleware))
        } else {
            router
        }
    }

    /// Apply rate limiting middleware
    pub fn with_rate_limiting(self, router: Router) -> Router {
        router.layer(axum::middleware::from_fn(rate_limit_middleware))
    }

    /// Apply CORS middleware
    pub fn with_cors(self, router: Router) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any);

        router.layer(cors)
    }
}

/// Custom request ID maker for MCP Gateway
#[derive(Clone, Copy)]
struct McpRequestIdMaker;

impl MakeRequestId for McpRequestIdMaker {
    fn make_request_id<B>(&mut self, _request: &axum::http::Request<B>) -> Option<RequestId> {
        let request_id = Uuid::new_v4().to_string();
        Some(RequestId::new(request_id.parse().ok()?))
    }
}

/// Authentication middleware function
async fn auth_middleware(request: Request, next: Next) -> Response {
    // TODO: Implement authentication logic
    next.run(request).await
}

/// Rate limiting middleware function
async fn rate_limit_middleware(request: Request, next: Next) -> Response {
    // TODO: Implement rate limiting logic
    next.run(request).await
}

/// Error handling middleware
pub async fn error_handler_middleware(request: Request, next: Next) -> Response {
    let response = next.run(request).await;

    // Add error handling logic here
    response
}

/// Security headers middleware
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;

    // Add security headers
    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );

    response
}

/// Request metrics middleware
pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    let start = std::time::Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status().as_u16();

    // Record metrics
    tracing::info!(
        method = %method,
        path = %path,
        status = %status,
        duration_ms = %duration.as_millis(),
        "request completed"
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CompressionConfig, RequestIdConfig, TimeoutConfig};
    use axum::{routing::get, Router};

    fn create_test_config() -> MiddlewareConfig {
        MiddlewareConfig {
            request_id: RequestIdConfig {
                enabled: true,
                header_name: "X-Request-ID".to_string(),
                generate_if_missing: true,
            },
            compression: CompressionConfig {
                enabled: true,
                algorithms: vec!["gzip".to_string()],
                min_size: 1024,
            },
            timeout: TimeoutConfig {
                request: Some(30),
                upstream: Some(30),
                keepalive: Some(60),
            },
            transform: vec![],
        }
    }

    #[test]
    fn test_middleware_stack_creation() {
        let config = create_test_config();
        let stack = MiddlewareStack::new(&config);
        assert!(stack.config.request_id.enabled);
        assert!(stack.config.compression.enabled);
    }

    #[test]
    fn test_middleware_application() {
        let config = create_test_config();
        let stack = MiddlewareStack::new(&config);

        let router = Router::new().route("/test", get(|| async { "test" }));
        let router_with_middleware = stack.apply(router);

        // Router should be modified with middleware
        assert_ne!(std::ptr::eq(
            &router_with_middleware as *const _,
            &router as *const _
        ));
    }

    #[test]
    fn test_request_id_maker() {
        let mut maker = McpRequestIdMaker;
        let request = axum::http::Request::builder().body(()).unwrap();

        let id1 = maker.make_request_id(&request);
        let id2 = maker.make_request_id(&request);

        assert!(id1.is_some());
        assert!(id2.is_some());
        // IDs should be different
        assert_ne!(format!("{:?}", id1), format!("{:?}", id2));
    }
}
