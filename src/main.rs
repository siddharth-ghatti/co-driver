use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod error;
mod gateway;

use config::Config;
use gateway::Gateway;

#[derive(Parser)]
#[command(name = "mcp-gateway")]
#[command(about = "A high-performance MCP (Model Context Protocol) Gateway")]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Server bind address
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    bind: SocketAddr,

    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Enable metrics endpoint
    #[arg(long, default_value = "true")]
    metrics: bool,

    /// Metrics bind address
    #[arg(long, default_value = "0.0.0.0:9090")]
    metrics_bind: SocketAddr,

    /// Enable development mode (more verbose logging)
    #[arg(long)]
    dev: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    init_tracing(&cli)?;

    info!("Starting MCP Gateway v{}", env!("CARGO_PKG_VERSION"));
    info!("Bind address: {}", cli.bind);
    info!("Metrics enabled: {}", cli.metrics);

    // Load configuration
    let config = Config::load(&cli.config).await.unwrap_or_else(|e| {
        warn!("Failed to load config file {}: {}", cli.config, e);
        warn!("Using default configuration");
        Config::default()
    });
    info!("Configuration loaded");

    // Create and start the gateway
    let gateway = Gateway::new(config, cli.bind, cli.metrics_bind).await?;

    // Start metrics server if enabled
    if cli.metrics {
        info!("Starting metrics server on: {}", cli.metrics_bind);
        gateway.start_metrics_server().await?;
    }

    // Start the main gateway server
    info!("Starting MCP Gateway server on: {}", cli.bind);
    gateway.start().await?;

    Ok(())
}

fn init_tracing(cli: &Cli) -> Result<()> {
    let log_level = cli
        .log_level
        .parse::<tracing::Level>()
        .map_err(|_| anyhow::anyhow!("Invalid log level: {}", cli.log_level))?;

    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            format!(
                "mcp_gateway={},tower_http=debug,axum::rejection=trace",
                log_level
            )
            .into()
        }),
    );

    if cli.dev {
        // Pretty console output for development
        subscriber
            .with(tracing_subscriber::fmt::layer().pretty())
            .try_init()?;
    } else {
        // Structured JSON output for production
        subscriber
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(false)
                    .with_span_list(true),
            )
            .try_init()?;
    }

    Ok(())
}
