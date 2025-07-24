use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::error::{GatewayError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub gateway: GatewayConfig,
    pub upstream: UpstreamConfig,
    pub security: SecurityConfig,
    pub metrics: MetricsConfig,
    pub logging: LoggingConfig,
    pub middleware: MiddlewareConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
    pub max_connections: Option<usize>,
    pub keepalive_timeout: Option<u64>,
    pub request_timeout: Option<u64>,
    pub graceful_shutdown_timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub name: String,
    pub version: String,
    pub environment: String,
    pub routes: Vec<RouteConfig>,
    pub load_balancing: LoadBalancingConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub retry: RetryConfig,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub id: String,
    pub path: String,
    pub method: Option<String>,
    pub upstream_group: String,
    pub timeout: Option<u64>,
    pub rate_limit: Option<RateLimitConfig>,
    pub auth_required: bool,
    pub transform: Option<TransformConfig>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub groups: HashMap<String, UpstreamGroup>,
    pub health_check: HealthCheckConfig,
    pub connection_pool: ConnectionPoolConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamGroup {
    pub servers: Vec<UpstreamServer>,
    pub strategy: LoadBalancingStrategy,
    pub health_check_override: Option<HealthCheckConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamServer {
    pub id: String,
    pub url: String,
    pub weight: Option<u32>,
    pub max_requests: Option<u32>,
    pub backup: bool,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    pub default_strategy: LoadBalancingStrategy,
    pub sticky_sessions: bool,
    pub session_cookie_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    IpHash,
    Random,
    WeightedRandom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    pub failure_threshold: u32,
    pub recovery_timeout: u64,
    pub min_request_threshold: u32,
    pub error_rate_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub enabled: bool,
    pub max_attempts: u32,
    pub backoff_strategy: BackoffStrategy,
    pub initial_delay: u64,
    pub max_delay: u64,
    pub multiplier: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    Fixed,
    Exponential,
    Linear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl: Option<u64>,
    pub max_size: Option<usize>,
    pub redis_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub auth: AuthConfig,
    pub cors: CorsConfig,
    pub tls: Option<TlsConfig>,
    pub rate_limiting: GlobalRateLimitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt_secret: Option<String>,
    pub jwt_issuer: Option<String>,
    pub jwt_audience: Option<String>,
    pub jwt_expiration: Option<u64>,
    pub api_key_header: Option<String>,
    pub basic_auth_users: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub enabled: bool,
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub max_age: Option<u64>,
    pub allow_credentials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_file: String,
    pub key_file: String,
    pub ca_file: Option<String>,
    pub verify_client: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalRateLimitConfig {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub key_strategy: RateLimitKeyStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub key_strategy: RateLimitKeyStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitKeyStrategy {
    IpAddress,
    UserId,
    ApiKey,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub enabled: bool,
    pub interval: u64,
    pub timeout: u64,
    pub path: String,
    pub method: String,
    pub expected_status: u16,
    pub healthy_threshold: u32,
    pub unhealthy_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    pub max_idle_per_host: Option<usize>,
    pub max_connections_per_host: Option<usize>,
    pub idle_timeout: Option<u64>,
    pub connect_timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub histogram_buckets: Option<Vec<f64>>,
    pub push_gateway: Option<PushGatewayConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushGatewayConfig {
    pub url: String,
    pub interval: u64,
    pub job_name: String,
    pub instance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
    pub output: LogOutput,
    pub file_path: Option<String>,
    pub max_file_size: Option<u64>,
    pub max_files: Option<u32>,
    pub structured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    Json,
    Text,
    Pretty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogOutput {
    Stdout,
    Stderr,
    File,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareConfig {
    pub request_id: RequestIdConfig,
    pub compression: CompressionConfig,
    pub timeout: TimeoutConfig,
    pub transform: Vec<TransformConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestIdConfig {
    pub enabled: bool,
    pub header_name: String,
    pub generate_if_missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub algorithms: Vec<String>,
    pub min_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    pub request: Option<u64>,
    pub upstream: Option<u64>,
    pub keepalive: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformConfig {
    pub name: String,
    pub enabled: bool,
    pub config: HashMap<String, serde_json::Value>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                workers: None,
                max_connections: Some(10000),
                keepalive_timeout: Some(30),
                request_timeout: Some(30),
                graceful_shutdown_timeout: Some(30),
            },
            gateway: GatewayConfig {
                name: "mcp-gateway".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                environment: "development".to_string(),
                routes: vec![],
                load_balancing: LoadBalancingConfig {
                    default_strategy: LoadBalancingStrategy::RoundRobin,
                    sticky_sessions: false,
                    session_cookie_name: None,
                },
                circuit_breaker: CircuitBreakerConfig {
                    enabled: true,
                    failure_threshold: 5,
                    recovery_timeout: 30,
                    min_request_threshold: 10,
                    error_rate_threshold: 0.5,
                },
                retry: RetryConfig {
                    enabled: true,
                    max_attempts: 3,
                    backoff_strategy: BackoffStrategy::Exponential,
                    initial_delay: 100,
                    max_delay: 5000,
                    multiplier: 2.0,
                },
                cache: CacheConfig {
                    enabled: false,
                    ttl: Some(300),
                    max_size: Some(1000),
                    redis_url: None,
                },
            },
            upstream: UpstreamConfig {
                groups: HashMap::new(),
                health_check: HealthCheckConfig {
                    enabled: true,
                    interval: 30,
                    timeout: 5,
                    path: "/health".to_string(),
                    method: "GET".to_string(),
                    expected_status: 200,
                    healthy_threshold: 2,
                    unhealthy_threshold: 3,
                },
                connection_pool: ConnectionPoolConfig {
                    max_idle_per_host: Some(32),
                    max_connections_per_host: Some(128),
                    idle_timeout: Some(90),
                    connect_timeout: Some(10),
                },
            },
            security: SecurityConfig {
                auth: AuthConfig {
                    enabled: false,
                    jwt_secret: None,
                    jwt_issuer: None,
                    jwt_audience: None,
                    jwt_expiration: Some(3600),
                    api_key_header: Some("X-API-Key".to_string()),
                    basic_auth_users: None,
                },
                cors: CorsConfig {
                    enabled: true,
                    allowed_origins: vec!["*".to_string()],
                    allowed_methods: vec![
                        "GET".to_string(),
                        "POST".to_string(),
                        "PUT".to_string(),
                        "DELETE".to_string(),
                    ],
                    allowed_headers: vec!["*".to_string()],
                    max_age: Some(3600),
                    allow_credentials: false,
                },
                tls: None,
                rate_limiting: GlobalRateLimitConfig {
                    enabled: false,
                    requests_per_minute: 1000,
                    burst_size: 100,
                    key_strategy: RateLimitKeyStrategy::IpAddress,
                },
            },
            metrics: MetricsConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 9090,
                path: "/metrics".to_string(),
                histogram_buckets: Some(vec![
                    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                ]),
                push_gateway: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: LogFormat::Json,
                output: LogOutput::Stdout,
                file_path: None,
                max_file_size: Some(100 * 1024 * 1024), // 100MB
                max_files: Some(10),
                structured: true,
            },
            middleware: MiddlewareConfig {
                request_id: RequestIdConfig {
                    enabled: true,
                    header_name: "X-Request-ID".to_string(),
                    generate_if_missing: true,
                },
                compression: CompressionConfig {
                    enabled: true,
                    algorithms: vec!["gzip".to_string(), "deflate".to_string()],
                    min_size: 1024,
                },
                timeout: TimeoutConfig {
                    request: Some(30),
                    upstream: Some(30),
                    keepalive: Some(60),
                },
                transform: vec![],
            },
        }
    }
}

impl Config {
    pub async fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            tracing::warn!("Config file {} not found, using defaults", path.display());
            return Ok(Self::default());
        }

        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            GatewayError::Config(config::ConfigError::Message(format!(
                "Failed to read config file: {}",
                e
            )))
        })?;

        let config = if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            toml::from_str(&content).map_err(|e| {
                GatewayError::Config(config::ConfigError::Message(format!(
                    "Failed to parse TOML config: {}",
                    e
                )))
            })?
        } else {
            // Try to parse as YAML/JSON
            serde_json::from_str(&content).map_err(|e| {
                GatewayError::Config(config::ConfigError::Message(format!(
                    "Failed to parse JSON config: {}",
                    e
                )))
            })?
        };

        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        // Validate server config
        if self.server.port == 0 {
            return Err(GatewayError::Validation(
                "Server port cannot be 0".to_string(),
            ));
        }

        // Validate upstream groups
        for (group_name, group) in &self.upstream.groups {
            if group.servers.is_empty() {
                return Err(GatewayError::Validation(format!(
                    "Upstream group '{}' has no servers",
                    group_name
                )));
            }

            for server in &group.servers {
                if server.url.is_empty() {
                    return Err(GatewayError::Validation(format!(
                        "Server '{}' has empty URL",
                        server.id
                    )));
                }
            }
        }

        // Validate routes
        for route in &self.gateway.routes {
            if route.path.is_empty() {
                return Err(GatewayError::Validation(format!(
                    "Route '{}' has empty path",
                    route.id
                )));
            }

            if !self.upstream.groups.contains_key(&route.upstream_group) {
                return Err(GatewayError::Validation(format!(
                    "Route '{}' references unknown upstream group '{}'",
                    route.id, route.upstream_group
                )));
            }
        }

        // Validate JWT config if auth is enabled
        if self.security.auth.enabled && self.security.auth.jwt_secret.is_none() {
            return Err(GatewayError::Validation(
                "JWT secret is required when authentication is enabled".to_string(),
            ));
        }

        Ok(())
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.server.request_timeout.unwrap_or(30))
    }

    pub fn keepalive_timeout(&self) -> Duration {
        Duration::from_secs(self.server.keepalive_timeout.unwrap_or(30))
    }

    pub fn graceful_shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.server.graceful_shutdown_timeout.unwrap_or(30))
    }

    pub fn health_check_interval(&self) -> Duration {
        Duration::from_secs(self.upstream.health_check.interval)
    }

    pub fn health_check_timeout(&self) -> Duration {
        Duration::from_secs(self.upstream.health_check.timeout)
    }

    pub fn circuit_breaker_recovery_timeout(&self) -> Duration {
        Duration::from_secs(self.gateway.circuit_breaker.recovery_timeout)
    }

    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.gateway.cache.ttl.unwrap_or(300))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_load_nonexistent_config() {
        let result = Config::load("nonexistent.toml").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_toml_config() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[server]
host = "127.0.0.1"
port = 3000

[gateway]
name = "test-gateway"
version = "1.0.0"
environment = "test"
routes = []

[gateway.load_balancing]
default_strategy = "round_robin"
sticky_sessions = false

[gateway.circuit_breaker]
enabled = true
failure_threshold = 3
recovery_timeout = 60
min_request_threshold = 5
error_rate_threshold = 0.3

[gateway.retry]
enabled = true
max_attempts = 2
backoff_strategy = "exponential"
initial_delay = 200
max_delay = 10000
multiplier = 3.0

[gateway.cache]
enabled = true
ttl = 600
max_size = 2000

[upstream]
groups = {}

[upstream.health_check]
enabled = true
interval = 15
timeout = 3
path = "/ping"
method = "GET"
expected_status = 200
healthy_threshold = 1
unhealthy_threshold = 2

[upstream.connection_pool]
max_idle_per_host = 16
max_connections_per_host = 64
idle_timeout = 45
connect_timeout = 5

[security.auth]
enabled = false

[security.cors]
enabled = true
allowed_origins = ["http://localhost:3000"]
allowed_methods = ["GET", "POST"]
allowed_headers = ["Content-Type"]
max_age = 1800
allow_credentials = true

[security.rate_limiting]
enabled = true
requests_per_minute = 500
burst_size = 50
key_strategy = "ip_address"

[metrics]
enabled = true
host = "127.0.0.1"
port = 8080
path = "/prometheus"

[logging]
level = "debug"
format = "pretty"
output = "stdout"
structured = false

[middleware.request_id]
enabled = true
header_name = "X-Trace-ID"
generate_if_missing = true

[middleware.compression]
enabled = true
algorithms = ["gzip"]
min_size = 2048

[middleware.timeout]
request = 60
upstream = 45
keepalive = 120
"#
        )
        .unwrap();

        let config = Config::load(file.path()).await.unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.gateway.name, "test-gateway");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_errors() {
        let mut config = Config::default();

        // Test invalid port
        config.server.port = 0;
        assert!(config.validate().is_err());

        // Reset and test empty upstream group
        config = Config::default();
        config.upstream.groups.insert(
            "empty".to_string(),
            UpstreamGroup {
                servers: vec![],
                strategy: LoadBalancingStrategy::RoundRobin,
                health_check_override: None,
            },
        );
        assert!(config.validate().is_err());
    }
}
