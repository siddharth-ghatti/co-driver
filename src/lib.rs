//! # MCP Gateway
//!
//! A high-performance gateway for the Model Context Protocol (MCP) written in Rust.
//!
//! This library provides components for building an MCP gateway that can route,
//! load balance, and manage connections to multiple MCP servers.

pub mod config;
pub mod error;
pub mod gateway;

// Re-export main types for convenience
pub use config::Config;
pub use error::{GatewayError, Result};
pub use gateway::Gateway;

/// Current version of the MCP Gateway
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// MCP Protocol version supported
pub const MCP_VERSION: &str = "2024-11-05";
