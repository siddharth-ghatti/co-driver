# Basic MCP Server Example

A simple Node.js-based MCP server that demonstrates the core Model Context Protocol features.

## Features

- **WebSocket Transport**: Full WebSocket-based MCP protocol implementation
- **Health Checks**: Built-in health check endpoint
- **Tools**: Example tools for basic operations (echo, add, time, server info)
- **Resources**: Server status, configuration, and connection information
- **Prompts**: Template prompts for common interactions
- **Connection Management**: Track and manage multiple client connections

## Quick Start

### Using Docker (Recommended)

The server is automatically included in the main docker-compose.yml. To run just this server:

```bash
docker run -p 3001:3001 node:18-alpine sh -c "
  cd /app && 
  npm install && 
  node server.js --port 3001
"
```

### Manual Installation

1. Install dependencies:
```bash
npm install
```

2. Start the server:
```bash
npm start
# or with custom port
node server.js --port 3001 --host 0.0.0.0
```

3. For development with auto-reload:
```bash
npm run dev
```

## Configuration

### Command Line Options

```bash
node server.js --help

Options:
  --port, -p  Port to listen on [number] [default: 3001]
  --host, -h  Host to bind to [string] [default: "0.0.0.0"]
  --help      Show help [boolean]
```

### Environment Variables

- `MCP_SERVER_ID`: Custom server identifier (default: `mcp-server-{port}`)
- `NODE_ENV`: Environment mode (development/production)

## API Reference

### WebSocket Endpoint

Connect to: `ws://localhost:3001/`

### Health Check Endpoint

```bash
curl http://localhost:3001/health
```

Response:
```json
{
  "status": "healthy",
  "serverId": "mcp-server-3001",
  "connections": 2,
  "uptime": 145.2,
  "timestamp": "2024-08-08T10:30:45.123Z"
}
```

## MCP Protocol Implementation

### Initialization

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
      "name": "example-client",
      "version": "1.0.0"
    }
  }
}
```

### Available Tools

#### echo
Echo back input text
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "echo",
    "arguments": {
      "text": "Hello, world!"
    }
  }
}
```

#### add
Add two numbers
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "add",
    "arguments": {
      "a": 10,
      "b": 5
    }
  }
}
```

#### current_time
Get current server time
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "current_time",
    "arguments": {}
  }
}
```

#### server_info
Get server information
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "name": "server_info",
    "arguments": {}
  }
}
```

### Available Resources

#### server://status
Current server status and statistics

#### server://config
Server configuration information

#### server://connections
List of active connections

### Available Prompts

#### greeting
A friendly greeting prompt
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "prompts/get",
  "params": {
    "name": "greeting",
    "arguments": {
      "name": "Alice"
    }
  }
}
```

#### system_status
System status report prompt

## Testing

### Manual Testing

1. Start the server:
```bash
npm start
```

2. Test with WebSocket client:
```javascript
const WebSocket = require('ws');
const ws = new WebSocket('ws://localhost:3001');

ws.on('open', () => {
  // Initialize connection
  ws.send(JSON.stringify({
    jsonrpc: '2.0',
    id: 1,
    method: 'initialize',
    params: {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'test-client', version: '1.0.0' }
    }
  }));
});

ws.on('message', (data) => {
  console.log('Received:', JSON.parse(data.toString()));
});
```

3. Test health endpoint:
```bash
curl http://localhost:3001/health
```

### Load Testing

Use the included test script:
```bash
node test.js
```

## Monitoring

### Metrics

The server provides basic metrics via the health endpoint:
- Connection count
- Uptime
- Memory usage
- CPU usage
- Message count per connection

### Logging

Structured logging to stdout with:
- Connection events
- Message handling
- Error tracking
- Performance metrics

## Production Deployment

### Docker

```dockerfile
FROM node:18-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production
COPY . .
EXPOSE 3001
CMD ["node", "server.js"]
```

### Environment Variables for Production

```bash
# Server configuration
MCP_SERVER_ID=mcp-server-prod-1
NODE_ENV=production

# Health check configuration  
HEALTH_CHECK_PORT=4001

# Monitoring
ENABLE_METRICS=true
LOG_LEVEL=info
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mcp-server
spec:
  replicas: 3
  selector:
    matchLabels:
      app: mcp-server
  template:
    metadata:
      labels:
        app: mcp-server
    spec:
      containers:
      - name: mcp-server
        image: mcp-server:latest
        ports:
        - containerPort: 3001
        env:
        - name: MCP_SERVER_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        livenessProbe:
          httpGet:
            path: /health
            port: 4001
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 4001
          initialDelaySeconds: 5
          periodSeconds: 5
```

## Troubleshooting

### Common Issues

#### Connection Refused
```bash
# Check if server is running
curl http://localhost:3001/health

# Check logs
npm start 2>&1 | grep ERROR
```

#### WebSocket Handshake Failed
- Verify the correct protocol (ws:// vs wss://)
- Check firewall settings
- Ensure port is not in use

#### High Memory Usage
- Monitor connection count
- Check for memory leaks in tools
- Review garbage collection settings

### Debug Mode

Enable debug logging:
```bash
DEBUG=mcp-server:* npm start
```

## Contributing

1. Add new tools in the `initializeTools()` method
2. Add new resources in the `initializeResources()` method  
3. Add new prompts in the `initializePrompts()` method
4. Update the corresponding handlers
5. Add tests for new functionality

## License

MIT License - see LICENSE file for details
