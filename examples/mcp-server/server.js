#!/usr/bin/env node

const WebSocket = require('ws');
const { v4: uuidv4 } = require('uuid');
const yargs = require('yargs');

// Parse command line arguments
const argv = yargs
  .option('port', {
    alias: 'p',
    description: 'Port to listen on',
    type: 'number',
    default: 3001
  })
  .option('host', {
    alias: 'h',
    description: 'Host to bind to',
    type: 'string',
    default: '0.0.0.0'
  })
  .help()
  .argv;

class MCPServer {
  constructor(port, host) {
    this.port = port;
    this.host = host;
    this.serverId = process.env.MCP_SERVER_ID || `mcp-server-${port}`;
    this.connections = new Map();
    this.tools = this.initializeTools();
    this.resources = this.initializeResources();
    this.prompts = this.initializePrompts();
  }

  initializeTools() {
    return [
      {
        name: "echo",
        description: "Echo back the input text",
        inputSchema: {
          type: "object",
          properties: {
            text: {
              type: "string",
              description: "Text to echo back"
            }
          },
          required: ["text"]
        }
      },
      {
        name: "add",
        description: "Add two numbers",
        inputSchema: {
          type: "object",
          properties: {
            a: {
              type: "number",
              description: "First number"
            },
            b: {
              type: "number",
              description: "Second number"
            }
          },
          required: ["a", "b"]
        }
      },
      {
        name: "current_time",
        description: "Get the current time",
        inputSchema: {
          type: "object",
          properties: {}
        }
      },
      {
        name: "server_info",
        description: "Get server information",
        inputSchema: {
          type: "object",
          properties: {}
        }
      }
    ];
  }

  initializeResources() {
    return [
      {
        uri: "server://status",
        name: "Server Status",
        description: "Current server status and statistics",
        mimeType: "application/json"
      },
      {
        uri: "server://config",
        name: "Server Configuration",
        description: "Current server configuration",
        mimeType: "application/json"
      },
      {
        uri: "server://connections",
        name: "Active Connections",
        description: "List of active connections",
        mimeType: "application/json"
      }
    ];
  }

  initializePrompts() {
    return [
      {
        name: "greeting",
        description: "A friendly greeting prompt",
        arguments: [
          {
            name: "name",
            description: "Name of the person to greet",
            required: true
          }
        ]
      },
      {
        name: "system_status",
        description: "System status report prompt",
        arguments: []
      }
    ];
  }

  start() {
    this.wss = new WebSocket.Server({ 
      port: this.port, 
      host: this.host,
      perMessageDeflate: false 
    });

    this.wss.on('connection', (ws, request) => {
      const connectionId = uuidv4();
      console.log(`[${this.serverId}] New connection: ${connectionId} from ${request.socket.remoteAddress}`);
      
      this.connections.set(connectionId, {
        ws,
        connectedAt: new Date(),
        lastActivity: new Date(),
        messageCount: 0
      });

      ws.on('message', async (data) => {
        try {
          const message = JSON.parse(data.toString());
          await this.handleMessage(connectionId, message);
        } catch (error) {
          console.error(`[${this.serverId}] Error handling message:`, error);
          this.sendError(ws, null, -32700, 'Parse error', error.message);
        }
      });

      ws.on('close', () => {
        console.log(`[${this.serverId}] Connection closed: ${connectionId}`);
        this.connections.delete(connectionId);
      });

      ws.on('error', (error) => {
        console.error(`[${this.serverId}] WebSocket error for ${connectionId}:`, error);
        this.connections.delete(connectionId);
      });
    });

    console.log(`[${this.serverId}] MCP Server listening on ${this.host}:${this.port}`);
    
    // Setup health check endpoint
    this.setupHealthCheck();
  }

  setupHealthCheck() {
    const http = require('http');
    const healthPort = this.port + 1000; // Health check on port + 1000
    
    const healthServer = http.createServer((req, res) => {
      if (req.url === '/health') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          status: 'healthy',
          serverId: this.serverId,
          connections: this.connections.size,
          uptime: process.uptime(),
          timestamp: new Date().toISOString()
        }));
      } else {
        res.writeHead(404);
        res.end('Not Found');
      }
    });

    healthServer.listen(healthPort, this.host, () => {
      console.log(`[${this.serverId}] Health check endpoint: http://${this.host}:${healthPort}/health`);
    });
  }

  async handleMessage(connectionId, message) {
    const connection = this.connections.get(connectionId);
    if (!connection) return;

    connection.lastActivity = new Date();
    connection.messageCount++;

    const { ws } = connection;
    const { jsonrpc, id, method, params } = message;

    console.log(`[${this.serverId}] Received: ${method} (${id}) from ${connectionId}`);

    try {
      switch (method) {
        case 'initialize':
          await this.handleInitialize(ws, id, params);
          break;
        case 'ping':
          await this.handlePing(ws, id);
          break;
        case 'tools/list':
          await this.handleToolsList(ws, id);
          break;
        case 'tools/call':
          await this.handleToolsCall(ws, id, params);
          break;
        case 'resources/list':
          await this.handleResourcesList(ws, id);
          break;
        case 'resources/read':
          await this.handleResourcesRead(ws, id, params);
          break;
        case 'prompts/list':
          await this.handlePromptsList(ws, id);
          break;
        case 'prompts/get':
          await this.handlePromptsGet(ws, id, params);
          break;
        default:
          this.sendError(ws, id, -32601, 'Method not found', `Unknown method: ${method}`);
      }
    } catch (error) {
      console.error(`[${this.serverId}] Error handling ${method}:`, error);
      this.sendError(ws, id, -32603, 'Internal error', error.message);
    }
  }

  async handleInitialize(ws, id, params) {
    const response = {
      jsonrpc: '2.0',
      id,
      result: {
        protocolVersion: '2024-11-05',
        capabilities: {
          tools: {
            listChanged: false
          },
          resources: {
            subscribe: false,
            listChanged: false
          },
          prompts: {
            listChanged: false
          }
        },
        serverInfo: {
          name: this.serverId,
          version: '1.0.0',
          description: 'Example MCP Server for demonstration'
        }
      }
    };

    this.sendMessage(ws, response);
  }

  async handlePing(ws, id) {
    this.sendMessage(ws, {
      jsonrpc: '2.0',
      id,
      result: {}
    });
  }

  async handleToolsList(ws, id) {
    this.sendMessage(ws, {
      jsonrpc: '2.0',
      id,
      result: {
        tools: this.tools
      }
    });
  }

  async handleToolsCall(ws, id, params) {
    const { name, arguments: args } = params;
    
    let result;
    switch (name) {
      case 'echo':
        result = {
          content: [
            {
              type: 'text',
              text: `Echo: ${args.text}`
            }
          ]
        };
        break;

      case 'add':
        const sum = args.a + args.b;
        result = {
          content: [
            {
              type: 'text',
              text: `${args.a} + ${args.b} = ${sum}`
            }
          ]
        };
        break;

      case 'current_time':
        result = {
          content: [
            {
              type: 'text',
              text: `Current time: ${new Date().toISOString()}`
            }
          ]
        };
        break;

      case 'server_info':
        result = {
          content: [
            {
              type: 'text',
              text: JSON.stringify({
                serverId: this.serverId,
                port: this.port,
                connections: this.connections.size,
                uptime: process.uptime(),
                nodeVersion: process.version,
                platform: process.platform,
                arch: process.arch
              }, null, 2)
            }
          ]
        };
        break;

      default:
        throw new Error(`Unknown tool: ${name}`);
    }

    this.sendMessage(ws, {
      jsonrpc: '2.0',
      id,
      result
    });
  }

  async handleResourcesList(ws, id) {
    this.sendMessage(ws, {
      jsonrpc: '2.0',
      id,
      result: {
        resources: this.resources
      }
    });
  }

  async handleResourcesRead(ws, id, params) {
    const { uri } = params;
    
    let content;
    switch (uri) {
      case 'server://status':
        content = JSON.stringify({
          serverId: this.serverId,
          status: 'running',
          connections: this.connections.size,
          uptime: process.uptime(),
          memoryUsage: process.memoryUsage(),
          cpuUsage: process.cpuUsage(),
          timestamp: new Date().toISOString()
        }, null, 2);
        break;

      case 'server://config':
        content = JSON.stringify({
          port: this.port,
          host: this.host,
          serverId: this.serverId,
          nodeVersion: process.version,
          platform: process.platform,
          arch: process.arch
        }, null, 2);
        break;

      case 'server://connections':
        const connectionsList = Array.from(this.connections.entries()).map(([id, conn]) => ({
          id,
          connectedAt: conn.connectedAt,
          lastActivity: conn.lastActivity,
          messageCount: conn.messageCount
        }));
        content = JSON.stringify(connectionsList, null, 2);
        break;

      default:
        throw new Error(`Unknown resource: ${uri}`);
    }

    this.sendMessage(ws, {
      jsonrpc: '2.0',
      id,
      result: {
        contents: [
          {
            uri,
            mimeType: 'application/json',
            text: content
          }
        ]
      }
    });
  }

  async handlePromptsList(ws, id) {
    this.sendMessage(ws, {
      jsonrpc: '2.0',
      id,
      result: {
        prompts: this.prompts
      }
    });
  }

  async handlePromptsGet(ws, id, params) {
    const { name, arguments: args } = params;
    
    let messages;
    switch (name) {
      case 'greeting':
        const userName = args?.name || 'there';
        messages = [
          {
            role: 'user',
            content: {
              type: 'text',
              text: `Hello ${userName}! Welcome to the ${this.serverId}. How can I help you today?`
            }
          }
        ];
        break;

      case 'system_status':
        const status = {
          serverId: this.serverId,
          connections: this.connections.size,
          uptime: Math.floor(process.uptime()),
          memoryUsage: process.memoryUsage(),
          timestamp: new Date().toISOString()
        };
        messages = [
          {
            role: 'user',
            content: {
              type: 'text',
              text: `System Status Report:\n\n${JSON.stringify(status, null, 2)}\n\nThe server is operating normally with ${status.connections} active connections.`
            }
          }
        ];
        break;

      default:
        throw new Error(`Unknown prompt: ${name}`);
    }

    this.sendMessage(ws, {
      jsonrpc: '2.0',
      id,
      result: {
        description: `Generated prompt: ${name}`,
        messages
      }
    });
  }

  sendMessage(ws, message) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(message));
    }
  }

  sendError(ws, id, code, message, data = null) {
    const errorMessage = {
      jsonrpc: '2.0',
      id,
      error: {
        code,
        message,
        data
      }
    };
    this.sendMessage(ws, errorMessage);
  }

  stop() {
    if (this.wss) {
      this.wss.close();
      console.log(`[${this.serverId}] Server stopped`);
    }
  }
}

// Start the server
const server = new MCPServer(argv.port, argv.host);
server.start();

// Graceful shutdown
process.on('SIGINT', () => {
  console.log('\nReceived SIGINT, shutting down gracefully...');
  server.stop();
  process.exit(0);
});

process.on('SIGTERM', () => {
  console.log('\nReceived SIGTERM, shutting down gracefully...');
  server.stop();
  process.exit(0);
});

module.exports = MCPServer;
