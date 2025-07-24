use crate::config::MetricsConfig;
use crate::error::{GatewayError, Result};
use crate::mcp::{McpClient, McpServer};
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, IntGaugeVec, Registry,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Server-specific metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerMetrics {
    pub server_id: String,
    pub requests_total: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub average_response_time_ms: f64,
    pub connections_active: u64,
    pub health_checks_total: u64,
    pub health_checks_success: u64,
    pub last_health_check: Option<u64>,
}

/// Gateway-wide metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct GatewayMetrics {
    pub total_requests: u64,
    pub active_connections: u64,
    pub upstream_servers: u64,
    pub healthy_servers: u64,
    pub request_rate_per_second: f64,
    pub average_response_time_ms: f64,
    pub error_rate_percentage: f64,
}

/// Metrics collector for the MCP Gateway
pub struct MetricsCollector {
    registry: Registry,
    config: MetricsConfig,

    // Request metrics
    requests_total: IntCounterVec,
    requests_duration: HistogramVec,
    requests_in_flight: IntGaugeVec,

    // Connection metrics
    connections_active: IntGauge,
    connections_total: IntCounter,

    // Upstream server metrics
    upstream_requests_total: IntCounterVec,
    upstream_requests_duration: HistogramVec,
    upstream_health_checks: IntCounterVec,
    upstream_servers_healthy: IntGaugeVec,

    // Gateway metrics
    gateway_uptime: Gauge,
    gateway_info: IntGaugeVec,

    // Circuit breaker metrics
    circuit_breaker_state: IntGaugeVec,
    circuit_breaker_trips: IntCounterVec,

    // Rate limiting metrics
    rate_limit_requests: IntCounterVec,
    rate_limit_rejected: IntCounterVec,

    // Cache metrics
    cache_hits: IntCounterVec,
    cache_misses: IntCounterVec,
    cache_size: IntGaugeVec,

    // Internal state
    server_metrics: Arc<RwLock<HashMap<String, ServerMetrics>>>,
    start_time: Instant,
    running: Arc<RwLock<bool>>,
}

impl MetricsCollector {
    pub fn new(config: &MetricsConfig) -> Result<Self> {
        let registry = Registry::new();

        // Request metrics
        let requests_total = IntCounterVec::new(
            prometheus::Opts::new(
                "mcp_gateway_requests_total",
                "Total number of requests processed by the gateway",
            ),
            &["method", "status", "route"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let requests_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "mcp_gateway_request_duration_seconds",
                "Time spent processing requests",
            )
            .buckets(
                config
                    .histogram_buckets
                    .as_ref()
                    .unwrap_or(&vec![
                        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                    ])
                    .clone(),
            ),
            &["method", "route"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let requests_in_flight = IntGaugeVec::new(
            prometheus::Opts::new(
                "mcp_gateway_requests_in_flight",
                "Number of requests currently being processed",
            ),
            &["method", "route"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        // Connection metrics
        let connections_active = IntGauge::new(
            "mcp_gateway_connections_active",
            "Number of active client connections",
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let connections_total = IntCounter::new(
            "mcp_gateway_connections_total",
            "Total number of client connections",
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        // Upstream server metrics
        let upstream_requests_total = IntCounterVec::new(
            prometheus::Opts::new(
                "mcp_gateway_upstream_requests_total",
                "Total number of requests sent to upstream servers",
            ),
            &["server_id", "method", "status"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let upstream_requests_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "mcp_gateway_upstream_request_duration_seconds",
                "Time spent on upstream requests",
            )
            .buckets(
                config
                    .histogram_buckets
                    .as_ref()
                    .unwrap_or(&vec![
                        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                    ])
                    .clone(),
            ),
            &["server_id", "method"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let upstream_health_checks = IntCounterVec::new(
            prometheus::Opts::new(
                "mcp_gateway_upstream_health_checks_total",
                "Total number of health checks performed on upstream servers",
            ),
            &["server_id", "status"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let upstream_servers_healthy = IntGaugeVec::new(
            prometheus::Opts::new(
                "mcp_gateway_upstream_servers_healthy",
                "Number of healthy upstream servers by group",
            ),
            &["group"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        // Gateway metrics
        let gateway_uptime = Gauge::new(
            "mcp_gateway_uptime_seconds",
            "Time since the gateway started",
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let gateway_info = IntGaugeVec::new(
            prometheus::Opts::new("mcp_gateway_info", "Gateway information"),
            &["version", "environment"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        // Circuit breaker metrics
        let circuit_breaker_state = IntGaugeVec::new(
            prometheus::Opts::new(
                "mcp_gateway_circuit_breaker_state",
                "Circuit breaker state (0=closed, 1=open, 2=half-open)",
            ),
            &["server_id"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let circuit_breaker_trips = IntCounterVec::new(
            prometheus::Opts::new(
                "mcp_gateway_circuit_breaker_trips_total",
                "Total number of circuit breaker trips",
            ),
            &["server_id"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        // Rate limiting metrics
        let rate_limit_requests = IntCounterVec::new(
            prometheus::Opts::new(
                "mcp_gateway_rate_limit_requests_total",
                "Total number of rate limit checks",
            ),
            &["key_type"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let rate_limit_rejected = IntCounterVec::new(
            prometheus::Opts::new(
                "mcp_gateway_rate_limit_rejected_total",
                "Total number of rate limited requests",
            ),
            &["key_type"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        // Cache metrics
        let cache_hits = IntCounterVec::new(
            prometheus::Opts::new("mcp_gateway_cache_hits_total", "Total number of cache hits"),
            &["cache_type"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let cache_misses = IntCounterVec::new(
            prometheus::Opts::new(
                "mcp_gateway_cache_misses_total",
                "Total number of cache misses",
            ),
            &["cache_type"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        let cache_size = IntGaugeVec::new(
            prometheus::Opts::new("mcp_gateway_cache_size", "Current cache size"),
            &["cache_type"],
        )
        .map_err(|e| GatewayError::Internal(format!("Failed to create metric: {}", e)))?;

        // Register all metrics
        registry
            .register(Box::new(requests_total.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(requests_duration.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(requests_in_flight.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(connections_active.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(connections_total.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(upstream_requests_total.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(upstream_requests_duration.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(upstream_health_checks.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(upstream_servers_healthy.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(gateway_uptime.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(gateway_info.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(circuit_breaker_state.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(circuit_breaker_trips.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(rate_limit_requests.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(rate_limit_rejected.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(cache_hits.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(cache_misses.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;
        registry
            .register(Box::new(cache_size.clone()))
            .map_err(|e| GatewayError::Internal(format!("Failed to register metric: {}", e)))?;

        Ok(Self {
            registry,
            config: config.clone(),
            requests_total,
            requests_duration,
            requests_in_flight,
            connections_active,
            connections_total,
            upstream_requests_total,
            upstream_requests_duration,
            upstream_health_checks,
            upstream_servers_healthy,
            gateway_uptime,
            gateway_info,
            circuit_breaker_state,
            circuit_breaker_trips,
            rate_limit_requests,
            rate_limit_rejected,
            cache_hits,
            cache_misses,
            cache_size,
            server_metrics: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start metrics collection
    pub async fn start_collection(
        &self,
        upstream_clients: Arc<RwLock<HashMap<String, Arc<McpClient>>>>,
        mcp_server: Arc<McpServer>,
    ) {
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        info!("Starting metrics collection");

        // Set gateway info
        self.gateway_info
            .with_label_values(&[env!("CARGO_PKG_VERSION"), "production"])
            .set(1);

        let collector = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            while *collector.running.read().await {
                interval.tick().await;

                if let Err(e) = collector
                    .collect_metrics(&upstream_clients, &mcp_server)
                    .await
                {
                    error!("Error collecting metrics: {}", e);
                }
            }

            info!("Metrics collection stopped");
        });
    }

    /// Stop metrics collection
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopping metrics collection");
        Ok(())
    }

    /// Collect current metrics
    async fn collect_metrics(
        &self,
        upstream_clients: &Arc<RwLock<HashMap<String, Arc<McpClient>>>>,
        mcp_server: &Arc<McpServer>,
    ) -> Result<()> {
        // Update gateway uptime
        self.gateway_uptime
            .set(self.start_time.elapsed().as_secs_f64());

        // Update active connections
        let active_connections = mcp_server.connection_count().await;
        self.connections_active.set(active_connections as i64);

        debug!(
            "Collected metrics: {} active connections",
            active_connections
        );

        Ok(())
    }

    /// Record a request
    pub async fn record_request(&self, method: &str, route: &str, status: u16, duration: Duration) {
        let status_str = status.to_string();
        self.requests_total
            .with_label_values(&[method, &status_str, route])
            .inc();
        self.requests_duration
            .with_label_values(&[method, route])
            .observe(duration.as_secs_f64());
    }

    /// Record request start
    pub async fn record_request_start(&self, method: &str, route: &str) {
        self.requests_in_flight
            .with_label_values(&[method, route])
            .inc();
    }

    /// Record request end
    pub async fn record_request_end(&self, method: &str, route: &str) {
        self.requests_in_flight
            .with_label_values(&[method, route])
            .dec();
    }

    /// Record new connection
    pub async fn record_connection(&self) {
        self.connections_total.inc();
    }

    /// Record upstream request
    pub async fn record_upstream_request(
        &self,
        server_id: &str,
        method: &str,
        status: u16,
        duration: Duration,
    ) {
        let status_str = status.to_string();
        self.upstream_requests_total
            .with_label_values(&[server_id, method, &status_str])
            .inc();
        self.upstream_requests_duration
            .with_label_values(&[server_id, method])
            .observe(duration.as_secs_f64());
    }

    /// Record health check success
    pub async fn record_health_check_success(&self, server_id: &str, duration: Duration) {
        self.upstream_health_checks
            .with_label_values(&[server_id, "success"])
            .inc();

        // Update server metrics
        let mut metrics = self.server_metrics.write().await;
        let server_metric = metrics
            .entry(server_id.to_string())
            .or_insert_with(|| ServerMetrics {
                server_id: server_id.to_string(),
                requests_total: 0,
                requests_success: 0,
                requests_failed: 0,
                average_response_time_ms: 0.0,
                connections_active: 0,
                health_checks_total: 0,
                health_checks_success: 0,
                last_health_check: None,
            });

        server_metric.health_checks_total += 1;
        server_metric.health_checks_success += 1;
        server_metric.last_health_check = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }

    /// Record health check failure
    pub async fn record_health_check_failure(&self, server_id: &str) {
        self.upstream_health_checks
            .with_label_values(&[server_id, "failure"])
            .inc();

        // Update server metrics
        let mut metrics = self.server_metrics.write().await;
        let server_metric = metrics
            .entry(server_id.to_string())
            .or_insert_with(|| ServerMetrics {
                server_id: server_id.to_string(),
                requests_total: 0,
                requests_success: 0,
                requests_failed: 0,
                average_response_time_ms: 0.0,
                connections_active: 0,
                health_checks_total: 0,
                health_checks_success: 0,
                last_health_check: None,
            });

        server_metric.health_checks_total += 1;
        server_metric.last_health_check = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }

    /// Set healthy servers count for a group
    pub async fn set_healthy_servers(&self, group: &str, count: i64) {
        self.upstream_servers_healthy
            .with_label_values(&[group])
            .set(count);
    }

    /// Record circuit breaker state
    pub async fn record_circuit_breaker_state(&self, server_id: &str, state: i64) {
        self.circuit_breaker_state
            .with_label_values(&[server_id])
            .set(state);
    }

    /// Record circuit breaker trip
    pub async fn record_circuit_breaker_trip(&self, server_id: &str) {
        self.circuit_breaker_trips
            .with_label_values(&[server_id])
            .inc();
    }

    /// Record rate limit check
    pub async fn record_rate_limit_check(&self, key_type: &str) {
        self.rate_limit_requests
            .with_label_values(&[key_type])
            .inc();
    }

    /// Record rate limit rejection
    pub async fn record_rate_limit_rejection(&self, key_type: &str) {
        self.rate_limit_rejected
            .with_label_values(&[key_type])
            .inc();
    }

    /// Record cache hit
    pub async fn record_cache_hit(&self, cache_type: &str) {
        self.cache_hits.with_label_values(&[cache_type]).inc();
    }

    /// Record cache miss
    pub async fn record_cache_miss(&self, cache_type: &str) {
        self.cache_misses.with_label_values(&[cache_type]).inc();
    }

    /// Set cache size
    pub async fn set_cache_size(&self, cache_type: &str, size: i64) {
        self.cache_size.with_label_values(&[cache_type]).set(size);
    }

    /// Export metrics in Prometheus format
    pub async fn export_metrics(&self) -> Result<String> {
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder
            .encode_to_string(&metric_families)
            .map_err(|e| GatewayError::Internal(format!("Failed to encode metrics: {}", e)))
    }

    /// Get server metrics
    pub async fn get_server_metrics(&self, server_id: &str) -> Option<ServerMetrics> {
        let metrics = self.server_metrics.read().await;
        metrics.get(server_id).cloned()
    }

    /// Get all server metrics
    pub async fn get_all_server_metrics(&self) -> HashMap<String, ServerMetrics> {
        let metrics = self.server_metrics.read().await;
        metrics.clone()
    }

    /// Get gateway metrics summary
    pub async fn get_gateway_metrics(&self) -> GatewayMetrics {
        let server_metrics = self.server_metrics.read().await;

        let total_requests: u64 = server_metrics.values().map(|m| m.requests_total).sum();
        let total_successes: u64 = server_metrics.values().map(|m| m.requests_success).sum();
        let total_failures: u64 = server_metrics.values().map(|m| m.requests_failed).sum();

        let error_rate_percentage = if total_requests > 0 {
            (total_failures as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        // Calculate average response time across all servers
        let avg_response_times: Vec<f64> = server_metrics
            .values()
            .filter(|m| m.average_response_time_ms > 0.0)
            .map(|m| m.average_response_time_ms)
            .collect();

        let average_response_time_ms = if !avg_response_times.is_empty() {
            avg_response_times.iter().sum::<f64>() / avg_response_times.len() as f64
        } else {
            0.0
        };

        // Calculate request rate (simplified)
        let uptime_secs = self.start_time.elapsed().as_secs() as f64;
        let request_rate_per_second = if uptime_secs > 0.0 {
            total_requests as f64 / uptime_secs
        } else {
            0.0
        };

        GatewayMetrics {
            total_requests,
            active_connections: 0, // This would be updated by the collection loop
            upstream_servers: server_metrics.len() as u64,
            healthy_servers: 0, // This would be provided by health checker
            request_rate_per_second,
            average_response_time_ms,
            error_rate_percentage,
        }
    }

    /// Reset metrics (for testing)
    #[cfg(test)]
    pub async fn reset_metrics(&self) {
        let mut metrics = self.server_metrics.write().await;
        metrics.clear();
    }
}

impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        Self {
            registry: Registry::new(), // Create new registry for clone
            config: self.config.clone(),
            requests_total: self.requests_total.clone(),
            requests_duration: self.requests_duration.clone(),
            requests_in_flight: self.requests_in_flight.clone(),
            connections_active: self.connections_active.clone(),
            connections_total: self.connections_total.clone(),
            upstream_requests_total: self.upstream_requests_total.clone(),
            upstream_requests_duration: self.upstream_requests_duration.clone(),
            upstream_health_checks: self.upstream_health_checks.clone(),
            upstream_servers_healthy: self.upstream_servers_healthy.clone(),
            gateway_uptime: self.gateway_uptime.clone(),
            gateway_info: self.gateway_info.clone(),
            circuit_breaker_state: self.circuit_breaker_state.clone(),
            circuit_breaker_trips: self.circuit_breaker_trips.clone(),
            rate_limit_requests: self.rate_limit_requests.clone(),
            rate_limit_rejected: self.rate_limit_rejected.clone(),
            cache_hits: self.cache_hits.clone(),
            cache_misses: self.cache_misses.clone(),
            cache_size: self.cache_size.clone(),
            server_metrics: self.server_metrics.clone(),
            start_time: self.start_time,
            running: self.running.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MetricsConfig;

    fn create_test_config() -> MetricsConfig {
        MetricsConfig {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 9090,
            path: "/metrics".to_string(),
            histogram_buckets: Some(vec![0.1, 0.5, 1.0]),
            push_gateway: None,
        }
    }

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config);
        assert!(collector.is_ok());
    }

    #[tokio::test]
    async fn test_record_request() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        collector
            .record_request("GET", "/api/test", 200, Duration::from_millis(100))
            .await;

        let metrics = collector.export_metrics().await.unwrap();
        assert!(metrics.contains("mcp_gateway_requests_total"));
    }

    #[tokio::test]
    async fn test_health_check_recording() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        collector
            .record_health_check_success("server1", Duration::from_millis(50))
            .await;
        collector.record_health_check_failure("server2").await;

        let server_metrics = collector.get_server_metrics("server1").await;
        assert!(server_metrics.is_some());
        assert_eq!(server_metrics.unwrap().health_checks_success, 1);
    }

    #[tokio::test]
    async fn test_gateway_metrics() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        let gateway_metrics = collector.get_gateway_metrics().await;
        assert_eq!(gateway_metrics.total_requests, 0);
        assert_eq!(gateway_metrics.upstream_servers, 0);
    }

    #[tokio::test]
    async fn test_metrics_export() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        collector.record_connection().await;
        let metrics = collector.export_metrics().await.unwrap();

        assert!(metrics.contains("mcp_gateway_connections_total"));
        assert!(metrics.contains("TYPE"));
        assert!(metrics.contains("HELP"));
    }
}
