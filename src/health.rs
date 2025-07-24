use crate::config::HealthCheckConfig;
use crate::error::{GatewayError, Result};
use crate::mcp::McpClient;
use crate::metrics::MetricsCollector;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Health status of a server
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

/// Health statistics for a server
#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthStats {
    pub status: String,
    pub consecutive_successes: u32,
    pub consecutive_failures: u32,
    pub total_checks: u64,
    pub total_successes: u64,
    pub total_failures: u64,
    pub last_check: Option<u64>,
    pub last_success: Option<u64>,
    pub last_failure: Option<u64>,
    pub average_response_time_ms: f64,
    pub uptime_percentage: f64,
}

/// Internal health state for a server
#[derive(Debug, Clone)]
struct ServerHealthState {
    status: HealthStatus,
    consecutive_successes: u32,
    consecutive_failures: u32,
    total_checks: u64,
    total_successes: u64,
    total_failures: u64,
    last_check: Option<Instant>,
    last_success: Option<Instant>,
    last_failure: Option<Instant>,
    response_times: Vec<Duration>,
    first_check: Option<Instant>,
}

impl Default for ServerHealthState {
    fn default() -> Self {
        Self {
            status: HealthStatus::Unknown,
            consecutive_successes: 0,
            consecutive_failures: 0,
            total_checks: 0,
            total_successes: 0,
            total_failures: 0,
            last_check: None,
            last_success: None,
            last_failure: None,
            response_times: Vec::new(),
            first_check: None,
        }
    }
}

impl ServerHealthState {
    fn record_success(&mut self, response_time: Duration) {
        let now = Instant::now();

        if self.first_check.is_none() {
            self.first_check = Some(now);
        }

        self.total_checks += 1;
        self.total_successes += 1;
        self.consecutive_successes += 1;
        self.consecutive_failures = 0;
        self.last_check = Some(now);
        self.last_success = Some(now);

        // Keep last 100 response times for averaging
        self.response_times.push(response_time);
        if self.response_times.len() > 100 {
            self.response_times.remove(0);
        }
    }

    fn record_failure(&mut self) {
        let now = Instant::now();

        if self.first_check.is_none() {
            self.first_check = Some(now);
        }

        self.total_checks += 1;
        self.total_failures += 1;
        self.consecutive_failures += 1;
        self.consecutive_successes = 0;
        self.last_check = Some(now);
        self.last_failure = Some(now);
    }

    fn to_stats(&self) -> HealthStats {
        let average_response_time_ms = if self.response_times.is_empty() {
            0.0
        } else {
            let total: Duration = self.response_times.iter().sum();
            total.as_millis() as f64 / self.response_times.len() as f64
        };

        let uptime_percentage = if self.total_checks == 0 {
            0.0
        } else {
            (self.total_successes as f64 / self.total_checks as f64) * 100.0
        };

        HealthStats {
            status: format!("{:?}", self.status),
            consecutive_successes: self.consecutive_successes,
            consecutive_failures: self.consecutive_failures,
            total_checks: self.total_checks,
            total_successes: self.total_successes,
            total_failures: self.total_failures,
            last_check: self.last_check.map(|t| t.elapsed().as_secs()),
            last_success: self.last_success.map(|t| t.elapsed().as_secs()),
            last_failure: self.last_failure.map(|t| t.elapsed().as_secs()),
            average_response_time_ms,
            uptime_percentage,
        }
    }
}

/// Health checker for monitoring upstream servers
pub struct HealthChecker {
    config: HealthCheckConfig,
    servers: Arc<RwLock<HashMap<String, Arc<McpClient>>>>,
    health_states: Arc<RwLock<HashMap<String, ServerHealthState>>>,
    running: Arc<RwLock<bool>>,
}

impl HealthChecker {
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            servers: Arc::new(RwLock::new(HashMap::new())),
            health_states: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Add a server to monitor
    pub async fn add_server(&self, server_id: String, client: Arc<McpClient>) {
        let mut servers = self.servers.write().await;
        servers.insert(server_id.clone(), client);

        let mut states = self.health_states.write().await;
        states.insert(server_id.clone(), ServerHealthState::default());

        info!("Added server to health monitoring: {}", server_id);
    }

    /// Remove a server from monitoring
    pub async fn remove_server(&self, server_id: &str) {
        let mut servers = self.servers.write().await;
        servers.remove(server_id);

        let mut states = self.health_states.write().await;
        states.remove(server_id);

        info!("Removed server from health monitoring: {}", server_id);
    }

    /// Start the health monitoring loop
    pub async fn start_monitoring(&self, metrics_collector: Arc<MetricsCollector>) {
        if !self.config.enabled {
            info!("Health checking is disabled");
            return;
        }

        {
            let mut running = self.running.write().await;
            *running = true;
        }

        info!(
            "Starting health monitoring with interval: {}s",
            self.config.interval
        );

        let checker = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(checker.config.interval));

            while *checker.running.read().await {
                interval.tick().await;

                if let Err(e) = checker.perform_health_checks(&metrics_collector).await {
                    error!("Error during health checks: {}", e);
                }
            }

            info!("Health monitoring stopped");
        });
    }

    /// Stop health monitoring
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopping health monitoring");
    }

    /// Perform health checks on all servers
    async fn perform_health_checks(&self, metrics_collector: &MetricsCollector) -> Result<()> {
        let servers = self.servers.read().await;
        let server_list: Vec<(String, Arc<McpClient>)> = servers
            .iter()
            .map(|(id, client)| (id.clone(), client.clone()))
            .collect();
        drop(servers);

        debug!("Performing health checks on {} servers", server_list.len());

        for (server_id, client) in server_list {
            if let Err(e) = self
                .check_server_health(&server_id, &client, metrics_collector)
                .await
            {
                warn!("Health check failed for {}: {}", server_id, e);
            }
        }

        Ok(())
    }

    /// Check health of a single server
    async fn check_server_health(
        &self,
        server_id: &str,
        client: &McpClient,
        metrics_collector: &MetricsCollector,
    ) -> Result<()> {
        debug!("Checking health of server: {}", server_id);

        let start_time = Instant::now();
        let health_result = self.perform_health_check(client).await;
        let response_time = start_time.elapsed();

        let mut states = self.health_states.write().await;
        let state = states.entry(server_id.to_string()).or_default();

        match health_result {
            Ok(_) => {
                state.record_success(response_time);

                // Update status based on consecutive successes
                if state.consecutive_successes >= self.config.healthy_threshold {
                    if state.status != HealthStatus::Healthy {
                        info!("Server {} is now healthy", server_id);
                        state.status = HealthStatus::Healthy;
                    }
                }

                metrics_collector
                    .record_health_check_success(server_id, response_time)
                    .await;
                debug!(
                    "Health check successful for {} ({}ms)",
                    server_id,
                    response_time.as_millis()
                );
            }
            Err(e) => {
                state.record_failure();

                // Update status based on consecutive failures
                if state.consecutive_failures >= self.config.unhealthy_threshold {
                    if state.status != HealthStatus::Unhealthy {
                        warn!("Server {} is now unhealthy", server_id);
                        state.status = HealthStatus::Unhealthy;
                    }
                }

                metrics_collector
                    .record_health_check_failure(server_id)
                    .await;
                debug!("Health check failed for {}: {}", server_id, e);
            }
        }

        Ok(())
    }

    /// Perform the actual health check
    async fn perform_health_check(&self, client: &McpClient) -> Result<()> {
        // Use a timeout for the health check
        let timeout_duration = Duration::from_secs(self.config.timeout);

        let health_check = async {
            match self.config.method.as_str() {
                "ping" => {
                    // Use MCP ping method
                    client.ping().await?;
                    Ok(())
                }
                "GET" => {
                    // For HTTP-based health checks, we could implement a simple GET request
                    // For now, we'll use ping as the default
                    client.ping().await?;
                    Ok(())
                }
                _ => {
                    // Default to ping
                    client.ping().await?;
                    Ok(())
                }
            }
        };

        tokio::time::timeout(timeout_duration, health_check)
            .await
            .map_err(|_| GatewayError::Timeout("Health check timeout".to_string()))?
    }

    /// Check if a specific server is healthy
    pub async fn is_server_healthy(&self, server_id: &str) -> bool {
        let states = self.health_states.read().await;
        states
            .get(server_id)
            .map(|state| state.status == HealthStatus::Healthy)
            .unwrap_or(false)
    }

    /// Get health stats for a server
    pub async fn get_server_stats(&self, server_id: &str) -> Option<HealthStats> {
        let states = self.health_states.read().await;
        states.get(server_id).map(|state| state.to_stats())
    }

    /// Get overall health status
    pub async fn overall_health(&self) -> bool {
        let states = self.health_states.read().await;

        if states.is_empty() {
            return true; // No servers to check
        }

        // Consider overall health as healthy if at least one server is healthy
        states
            .values()
            .any(|state| state.status == HealthStatus::Healthy)
    }

    /// Get count of healthy servers
    pub async fn healthy_server_count(&self) -> usize {
        let states = self.health_states.read().await;
        states
            .values()
            .filter(|state| state.status == HealthStatus::Healthy)
            .count()
    }

    /// Get all server health statuses
    pub async fn get_all_server_health(&self) -> HashMap<String, HealthStats> {
        let states = self.health_states.read().await;
        states
            .iter()
            .map(|(id, state)| (id.clone(), state.to_stats()))
            .collect()
    }

    /// Get list of healthy servers
    pub async fn get_healthy_servers(&self) -> Vec<String> {
        let states = self.health_states.read().await;
        states
            .iter()
            .filter(|(_, state)| state.status == HealthStatus::Healthy)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get list of unhealthy servers
    pub async fn get_unhealthy_servers(&self) -> Vec<String> {
        let states = self.health_states.read().await;
        states
            .iter()
            .filter(|(_, state)| state.status == HealthStatus::Unhealthy)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Force a health check on a specific server
    pub async fn force_check(
        &self,
        server_id: &str,
        metrics_collector: &MetricsCollector,
    ) -> Result<bool> {
        let servers = self.servers.read().await;
        if let Some(client) = servers.get(server_id) {
            let client = client.clone();
            drop(servers);

            self.check_server_health(server_id, &client, metrics_collector)
                .await?;
            Ok(self.is_server_healthy(server_id).await)
        } else {
            Err(GatewayError::NotFound(format!("Server {}", server_id)))
        }
    }

    /// Get health check configuration
    pub fn config(&self) -> &HealthCheckConfig {
        &self.config
    }

    /// Update health check configuration
    pub async fn update_config(&mut self, new_config: HealthCheckConfig) {
        self.config = new_config;
        info!("Health check configuration updated");
    }
}

impl Clone for HealthChecker {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            servers: self.servers.clone(),
            health_states: self.health_states.clone(),
            running: self.running.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HealthCheckConfig;
    use std::time::Duration;

    fn create_test_config() -> HealthCheckConfig {
        HealthCheckConfig {
            enabled: true,
            interval: 5,
            timeout: 3,
            path: "/health".to_string(),
            method: "ping".to_string(),
            expected_status: 200,
            healthy_threshold: 2,
            unhealthy_threshold: 3,
        }
    }

    #[tokio::test]
    async fn test_health_checker_creation() {
        let config = create_test_config();
        let checker = HealthChecker::new(config);

        assert!(checker.config.enabled);
        assert_eq!(checker.config.interval, 5);
        assert_eq!(checker.healthy_server_count().await, 0);
    }

    #[tokio::test]
    async fn test_server_health_state() {
        let mut state = ServerHealthState::default();

        // Test initial state
        assert_eq!(state.status, HealthStatus::Unknown);
        assert_eq!(state.total_checks, 0);

        // Test recording success
        state.record_success(Duration::from_millis(100));
        assert_eq!(state.total_checks, 1);
        assert_eq!(state.total_successes, 1);
        assert_eq!(state.consecutive_successes, 1);

        // Test recording failure
        state.record_failure();
        assert_eq!(state.total_checks, 2);
        assert_eq!(state.total_failures, 1);
        assert_eq!(state.consecutive_failures, 1);
        assert_eq!(state.consecutive_successes, 0);
    }

    #[tokio::test]
    async fn test_health_stats_conversion() {
        let mut state = ServerHealthState::default();

        // Record some data
        state.record_success(Duration::from_millis(100));
        state.record_success(Duration::from_millis(150));
        state.record_failure();

        let stats = state.to_stats();
        assert_eq!(stats.total_checks, 3);
        assert_eq!(stats.total_successes, 2);
        assert_eq!(stats.total_failures, 1);
        assert_eq!(stats.consecutive_failures, 1);
        assert!((stats.uptime_percentage - 66.67).abs() < 0.1);
        assert!((stats.average_response_time_ms - 125.0).abs() < 0.1);
    }

    #[test]
    fn test_health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_eq!(HealthStatus::Unhealthy, HealthStatus::Unhealthy);
        assert_eq!(HealthStatus::Unknown, HealthStatus::Unknown);

        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unknown);
        assert_ne!(HealthStatus::Unhealthy, HealthStatus::Unknown);
    }
}
