# MCP Server Deployment Examples

This directory contains example MCP servers and deployment configurations that work with the MCP Gateway. These examples demonstrate how to create, configure, and deploy various types of MCP servers.

## Available Examples

### 1. Basic MCP Server (`mcp-server/`)
A simple Node.js-based MCP server that implements basic MCP protocol features:
- Ping/pong health checks
- Basic tool listing
- Simple resource access
- WebSocket transport

### 2. File Tools Server (`file-tools-server/`)
An MCP server that provides file system operations:
- File reading and writing
- Directory listing
- File search capabilities
- File metadata access

### 3. Web Tools Server (`web-tools-server/`)
An MCP server that provides web-related tools:
- HTTP requests
- Web scraping
- URL validation
- Content extraction

### 4. Python MCP Server (`python-server/`)
A Python-based MCP server demonstrating:
- Python MCP SDK usage
- Data processing tools
- Scientific computing capabilities
- Async/await patterns

### 5. Rust MCP Server (`rust-server/`)
A Rust-based MCP server showing:
- High-performance implementation
- Memory-safe operations
- Concurrent request handling
- Low-level system access

## Quick Start

### Using Docker Compose (Recommended)

1. Start all example servers with the gateway:
```bash
docker-compose up -d
```

2. Test the servers through the gateway:
```bash
# Test basic server
curl -H "Upgrade: websocket" http://localhost:8080/mcp

# Test file tools
curl http://localhost:8080/tools/files

# Test web tools
curl http://localhost:8080/tools/web
```

### Manual Deployment

Each server directory contains its own deployment instructions. See the README.md in each subdirectory for specific setup steps.

## Configuration

The MCP Gateway is configured to route to these servers via `config.toml`:

```toml
# Basic MCP servers (load balanced)
[upstream.groups.default]
strategy = "round_robin"

[[upstream.groups.default.servers]]
id = "mcp-server-1"
url = "ws://localhost:3001"

[[upstream.groups.default.servers]]
id = "mcp-server-2"
url = "ws://localhost:3002"

# Tool-specific servers
[upstream.groups.tools]
strategy = "weighted_round_robin"

[[upstream.groups.tools.servers]]
id = "file-tools"
url = "ws://localhost:4001"
weight = 200

[[upstream.groups.tools.servers]]
id = "web-tools"
url = "ws://localhost:4002"
weight = 100
```

## Deployment Strategies

### Development Environment
- Use Docker Compose for local development
- Enable hot-reload and debug logging
- Use volume mounts for code changes

### Production Environment
- Use Kubernetes for orchestration
- Enable health checks and auto-scaling
- Use proper secrets management
- Configure monitoring and logging

### Cloud Deployment
- AWS ECS/Fargate
- Google Cloud Run
- Azure Container Instances
- Digital Ocean App Platform

## Monitoring

All example servers include:
- Health check endpoints
- Prometheus metrics
- Structured logging
- Error tracking

## Security

Example configurations include:
- Authentication middleware
- Rate limiting
- Input validation
- Security headers

## Contributing

To add a new example server:

1. Create a new directory under `examples/`
2. Include a README.md with setup instructions
3. Add Docker configuration
4. Update the main docker-compose.yml
5. Add corresponding gateway configuration
6. Include tests and documentation
