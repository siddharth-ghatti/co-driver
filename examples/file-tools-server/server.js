#!/usr/bin/env node

const WebSocket = require('ws');
const { v4: uuidv4 } = require('uuid');
const yargs = require('yargs');
const fs = require('fs').promises;
const path = require('path');
const mime = require('mime-types');
const chokidar = require('chokidar');

// Parse command line arguments
const argv = yargs
  .option('port', {
    alias: 'p',
    description: 'Port to listen on',
    type: 'number',
    default: 4001
  })
  .option('host', {
    alias: 'h',
    description: 'Host to bind to',
    type: 'string',
    default: '0.0.0.0'
  })
  .option('base-path', {
    alias: 'b',
    description: 'Base path for file operations',
    type: 'string',
    default: process.env.SHARED_PATH || '/tmp/mcp-files'
  })
  .help()
  .argv;

class FileToolsMCPServer {
  constructor(port, host, basePath) {
    this.port = port;
    this.host = host;
    this.basePath = path.resolve(basePath);
    this.serverId = process.env.MCP_SERVER_ID || `file-tools-${port}`;
    this.connections = new Map();
    this.fileWatchers = new Map();
    this.tools = this.initializeTools();
    this.resources = this.initializeResources();
    
    // Ensure base directory exists
    this.ensureBaseDirectory();
  }

  async ensureBaseDirectory() {
    try {
      await fs.access(this.basePath);
    } catch {
      await fs.mkdir(this.basePath, { recursive: true });
      console.log(`[${this.serverId}] Created base directory: ${this.basePath}`);
    }
  }

  initializeTools() {
    return [
      {
        name: "read_file",
        description: "Read the contents of a file",
        inputSchema: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description: "Path to the file to read (relative to base path)"
            },
            encoding: {
              type: "string",
              description: "File encoding (default: utf8)",
              enum: ["utf8", "ascii", "base64", "hex"],
              default: "utf8"
            }
          },
          required: ["path"]
        }
      },
      {
        name: "write_file",
        description: "Write content to a file",
        inputSchema: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description: "Path to the file to write (relative to base path)"
            },
            content: {
              type: "string",
              description: "Content to write to the file"
            },
            encoding: {
              type: "string",
              description: "File encoding (default: utf8)",
              enum: ["utf8", "ascii", "base64", "hex"],
              default: "utf8"
            },
            mode: {
              type: "string",
              description: "Write mode",
              enum: ["overwrite", "append"],
              default: "overwrite"
            }
          },
          required: ["path", "content"]
        }
      },
      {
        name: "list_directory",
        description: "List the contents of a directory",
        inputSchema: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description: "Path to the directory (relative to base path)",
              default: "."
            },
            recursive: {
              type: "boolean",
              description: "List recursively",
              default: false
            },
            include_hidden: {
              type: "boolean",
              description: "Include hidden files",
              default: false
            }
          }
        }
      },
      {
        name: "create_directory",
        description: "Create a new directory",
        inputSchema: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description: "Path to the directory to create (relative to base path)"
            },
            recursive: {
              type: "boolean",
              description: "Create parent directories if they don't exist",
              default: true
            }
          },
          required: ["path"]
        }
      },
      {
        name: "delete_file",
        description: "Delete a file or directory",
        inputSchema: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description: "Path to the file or directory to delete (relative to base path)"
            },
            recursive: {
              type: "boolean",
              description: "Delete recursively (for directories)",
              default: false
            }
          },
          required: ["path"]
        }
      },
      {
        name: "copy_file",
        description: "Copy a file or directory",
        inputSchema: {
          type: "object",
          properties: {
            source: {
              type: "string",
              description: "Source path (relative to base path)"
            },
            destination: {
              type: "string",
              description: "Destination path (relative to base path)"
            },
            overwrite: {
              type: "boolean",
              description: "Overwrite if destination exists",
              default: false
            }
          },
          required: ["source", "destination"]
        }
      },
      {
        name: "move_file",
        description: "Move/rename a file or directory",
        inputSchema: {
          type: "object",
          properties: {
            source: {
              type: "string",
              description: "Source path (relative to base path)"
            },
            destination: {
              type: "string",
              description: "Destination path (relative to base path)"
            },
            overwrite: {
              type: "boolean",
              description: "Overwrite if destination exists",
              default: false
            }
          },
          required: ["source", "destination"]
        }
      },
      {
        name: "get_file_info",
        description: "Get information about a file or directory",
        inputSchema: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description: "Path to the file or directory (relative to base path)"
            }
          },
          required: ["path"]
        }
      },
      {
        name: "search_files",
        description: "Search for files matching a pattern",
        inputSchema: {
          type: "object",
          properties: {
            pattern: {
              type: "string",
              description: "Search pattern (supports glob patterns)"
            },
            path: {
              type: "string",
              description: "Directory to search in (relative to base path)",
              default: "."
            },
            content_search: {
              type: "string",
              description: "Search within file contents (regex pattern)"
            },
            max_results: {
              type: "number",
              description: "Maximum number of results",
              default: 100
            }
          },
          required: ["pattern"]
        }
      },
      {
        name: "watch_file",
        description: "Watch a file or directory for changes",
        inputSchema: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description: "Path to watch (relative to base path)"
            },
            events: {
              type: "array",
              items: {
                type: "string",
                enum: ["add", "change", "unlink", "addDir", "unlinkDir"]
              },
              description: "Events to watch for",
              default: ["change"]
            }
          },
          required: ["path"]
        }
      }
    ];
  }

  initializeResources() {
    return [
      {
        uri: "file://directory-tree",
        name: "Directory Tree",
        description: "Complete directory tree structure",
        mimeType: "application/json"
      },
      {
        uri: "file://disk-usage",
        name: "Disk Usage",
        description: "Disk usage statistics",
        mimeType: "application/json"
      },
      {
        uri: "file://recent-files",
        name: "Recent Files",
        description: "Recently modified files",
        mimeType: "application/json"
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
        messageCount: 0,
        watchers: new Set()
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
        this.cleanupConnection(connectionId);
      });

      ws.on('error', (error) => {
        console.error(`[${this.serverId}] WebSocket error for ${connectionId}:`, error);
        this.cleanupConnection(connectionId);
      });
    });

    console.log(`[${this.serverId}] File Tools MCP Server listening on ${this.host}:${this.port}`);
    console.log(`[${this.serverId}] Base path: ${this.basePath}`);
    
    this.setupHealthCheck();
  }

  cleanupConnection(connectionId) {
    const connection = this.connections.get(connectionId);
    if (connection) {
      // Stop all watchers for this connection
      connection.watchers.forEach(watcherId => {
        const watcher = this.fileWatchers.get(watcherId);
        if (watcher) {
          watcher.close();
          this.fileWatchers.delete(watcherId);
        }
      });
      this.connections.delete(connectionId);
    }
  }

  setupHealthCheck() {
    const http = require('http');
    const healthPort = this.port + 1000;
    
    const healthServer = http.createServer(async (req, res) => {
      if (req.url === '/health') {
        try {
          const stats = await fs.stat(this.basePath);
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({
            status: 'healthy',
            serverId: this.serverId,
            connections: this.connections.size,
            basePath: this.basePath,
            basePathExists: stats.isDirectory(),
            activeWatchers: this.fileWatchers.size,
            uptime: process.uptime(),
            timestamp: new Date().toISOString()
          }));
        } catch (error) {
          res.writeHead(500, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({
            status: 'unhealthy',
            error: error.message,
            timestamp: new Date().toISOString()
          }));
        }
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
          await this.handleToolsCall(ws, id, params, connectionId);
          break;
        case 'resources/list':
          await this.handleResourcesList(ws, id);
          break;
        case 'resources/read':
          await this.handleResourcesRead(ws, id, params);
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
          }
        },
        serverInfo: {
          name: this.serverId,
          version: '1.0.0',
          description: 'File Tools MCP Server - provides file system operations'
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

  async handleToolsCall(ws, id, params, connectionId) {
    const { name, arguments: args } = params;
    
    let result;
    switch (name) {
      case 'read_file':
        result = await this.readFile(args);
        break;
      case 'write_file':
        result = await this.writeFile(args);
        break;
      case 'list_directory':
        result = await this.listDirectory(args);
        break;
      case 'create_directory':
        result = await this.createDirectory(args);
        break;
      case 'delete_file':
        result = await this.deleteFile(args);
        break;
      case 'copy_file':
        result = await this.copyFile(args);
        break;
      case 'move_file':
        result = await this.moveFile(args);
        break;
      case 'get_file_info':
        result = await this.getFileInfo(args);
        break;
      case 'search_files':
        result = await this.searchFiles(args);
        break;
      case 'watch_file':
        result = await this.watchFile(args, connectionId);
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

  async readFile(args) {
    const filePath = this.resolvePath(args.path);
    const encoding = args.encoding || 'utf8';
    
    try {
      const content = await fs.readFile(filePath, encoding);
      const stats = await fs.stat(filePath);
      
      return {
        content: [
          {
            type: 'text',
            text: content
          }
        ],
        metadata: {
          path: args.path,
          size: stats.size,
          modified: stats.mtime.toISOString(),
          encoding: encoding,
          mimeType: mime.lookup(filePath) || 'application/octet-stream'
        }
      };
    } catch (error) {
      throw new Error(`Failed to read file: ${error.message}`);
    }
  }

  async writeFile(args) {
    const filePath = this.resolvePath(args.path);
    const encoding = args.encoding || 'utf8';
    const mode = args.mode || 'overwrite';
    
    try {
      // Ensure directory exists
      await fs.mkdir(path.dirname(filePath), { recursive: true });
      
      if (mode === 'append') {
        await fs.appendFile(filePath, args.content, encoding);
      } else {
        await fs.writeFile(filePath, args.content, encoding);
      }
      
      const stats = await fs.stat(filePath);
      
      return {
        content: [
          {
            type: 'text',
            text: `File written successfully: ${args.path}`
          }
        ],
        metadata: {
          path: args.path,
          size: stats.size,
          modified: stats.mtime.toISOString(),
          mode: mode
        }
      };
    } catch (error) {
      throw new Error(`Failed to write file: ${error.message}`);
    }
  }

  async listDirectory(args) {
    const dirPath = this.resolvePath(args.path || '.');
    const recursive = args.recursive || false;
    const includeHidden = args.include_hidden || false;
    
    try {
      const items = await this.listDirectoryRecursive(dirPath, recursive, includeHidden);
      
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(items, null, 2)
          }
        ],
        metadata: {
          path: args.path || '.',
          itemCount: items.length,
          recursive: recursive,
          includeHidden: includeHidden
        }
      };
    } catch (error) {
      throw new Error(`Failed to list directory: ${error.message}`);
    }
  }

  async listDirectoryRecursive(dirPath, recursive, includeHidden) {
    const items = [];
    const entries = await fs.readdir(dirPath);
    
    for (const entry of entries) {
      if (!includeHidden && entry.startsWith('.')) continue;
      
      const fullPath = path.join(dirPath, entry);
      const relativePath = path.relative(this.basePath, fullPath);
      const stats = await fs.stat(fullPath);
      
      const item = {
        name: entry,
        path: relativePath,
        type: stats.isDirectory() ? 'directory' : 'file',
        size: stats.size,
        modified: stats.mtime.toISOString(),
        permissions: stats.mode.toString(8)
      };
      
      if (stats.isFile()) {
        item.mimeType = mime.lookup(fullPath) || 'application/octet-stream';
      }
      
      items.push(item);
      
      if (recursive && stats.isDirectory()) {
        const children = await this.listDirectoryRecursive(fullPath, true, includeHidden);
        items.push(...children);
      }
    }
    
    return items;
  }

  async createDirectory(args) {
    const dirPath = this.resolvePath(args.path);
    const recursive = args.recursive !== false;
    
    try {
      await fs.mkdir(dirPath, { recursive });
      
      return {
        content: [
          {
            type: 'text',
            text: `Directory created successfully: ${args.path}`
          }
        ],
        metadata: {
          path: args.path,
          recursive: recursive
        }
      };
    } catch (error) {
      throw new Error(`Failed to create directory: ${error.message}`);
    }
  }

  async deleteFile(args) {
    const filePath = this.resolvePath(args.path);
    const recursive = args.recursive || false;
    
    try {
      const stats = await fs.stat(filePath);
      
      if (stats.isDirectory()) {
        await fs.rmdir(filePath, { recursive });
      } else {
        await fs.unlink(filePath);
      }
      
      return {
        content: [
          {
            type: 'text',
            text: `${stats.isDirectory() ? 'Directory' : 'File'} deleted successfully: ${args.path}`
          }
        ],
        metadata: {
          path: args.path,
          type: stats.isDirectory() ? 'directory' : 'file',
          recursive: recursive
        }
      };
    } catch (error) {
      throw new Error(`Failed to delete: ${error.message}`);
    }
  }

  async copyFile(args) {
    const sourcePath = this.resolvePath(args.source);
    const destPath = this.resolvePath(args.destination);
    
    try {
      // Check if destination exists and overwrite flag
      try {
        await fs.access(destPath);
        if (!args.overwrite) {
          throw new Error('Destination exists and overwrite is false');
        }
      } catch {
        // Destination doesn't exist, which is fine
      }
      
      // Ensure destination directory exists
      await fs.mkdir(path.dirname(destPath), { recursive: true });
      
      await fs.copyFile(sourcePath, destPath);
      
      return {
        content: [
          {
            type: 'text',
            text: `File copied successfully: ${args.source} → ${args.destination}`
          }
        ],
        metadata: {
          source: args.source,
          destination: args.destination,
          overwrite: args.overwrite || false
        }
      };
    } catch (error) {
      throw new Error(`Failed to copy file: ${error.message}`);
    }
  }

  async moveFile(args) {
    const sourcePath = this.resolvePath(args.source);
    const destPath = this.resolvePath(args.destination);
    
    try {
      // Check if destination exists and overwrite flag
      try {
        await fs.access(destPath);
        if (!args.overwrite) {
          throw new Error('Destination exists and overwrite is false');
        }
      } catch {
        // Destination doesn't exist, which is fine
      }
      
      // Ensure destination directory exists
      await fs.mkdir(path.dirname(destPath), { recursive: true });
      
      await fs.rename(sourcePath, destPath);
      
      return {
        content: [
          {
            type: 'text',
            text: `File moved successfully: ${args.source} → ${args.destination}`
          }
        ],
        metadata: {
          source: args.source,
          destination: args.destination,
          overwrite: args.overwrite || false
        }
      };
    } catch (error) {
      throw new Error(`Failed to move file: ${error.message}`);
    }
  }

  async getFileInfo(args) {
    const filePath = this.resolvePath(args.path);
    
    try {
      const stats = await fs.stat(filePath);
      const info = {
        path: args.path,
        absolutePath: filePath,
        type: stats.isDirectory() ? 'directory' : 'file',
        size: stats.size,
        created: stats.birthtime.toISOString(),
        modified: stats.mtime.toISOString(),
        accessed: stats.atime.toISOString(),
        permissions: stats.mode.toString(8),
        isReadable: true, // We'll assume readable if we can stat it
        isWritable: true  // We'll assume writable for simplicity
      };
      
      if (stats.isFile()) {
        info.mimeType = mime.lookup(filePath) || 'application/octet-stream';
      }
      
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(info, null, 2)
          }
        ],
        metadata: info
      };
    } catch (error) {
      throw new Error(`Failed to get file info: ${error.message}`);
    }
  }

  async searchFiles(args) {
    const searchPath = this.resolvePath(args.path || '.');
    const pattern = args.pattern;
    const contentSearch = args.content_search;
    const maxResults = args.max_results || 100;
    
    try {
      const results = [];
      await this.searchFilesRecursive(searchPath, pattern, contentSearch, results, maxResults);
      
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(results, null, 2)
          }
        ],
        metadata: {
          pattern: pattern,
          contentSearch: contentSearch,
          searchPath: args.path || '.',
          resultCount: results.length,
          maxResults: maxResults
        }
      };
    } catch (error) {
      throw new Error(`Failed to search files: ${error.message}`);
    }
  }

  async searchFilesRecursive(dirPath, pattern, contentSearch, results, maxResults) {
    if (results.length >= maxResults) return;
    
    const entries = await fs.readdir(dirPath);
    const globPattern = new RegExp(pattern.replace(/\*/g, '.*').replace(/\?/g, '.'));
    
    for (const entry of entries) {
      if (results.length >= maxResults) break;
      
      const fullPath = path.join(dirPath, entry);
      const relativePath = path.relative(this.basePath, fullPath);
      const stats = await fs.stat(fullPath);
      
      // Check filename pattern match
      if (globPattern.test(entry)) {
        const match = {
          path: relativePath,
          name: entry,
          type: stats.isDirectory() ? 'directory' : 'file',
          size: stats.size,
          modified: stats.mtime.toISOString(),
          matchType: 'filename'
        };
        
        // If content search is specified and this is a file, search content
        if (contentSearch && stats.isFile()) {
          try {
            const content = await fs.readFile(fullPath, 'utf8');
            const contentRegex = new RegExp(contentSearch, 'gi');
            const matches = content.match(contentRegex);
            if (matches) {
              match.matchType = 'content';
              match.contentMatches = matches.length;
            }
          } catch {
            // Skip files that can't be read as text
          }
        }
        
        results.push(match);
      }
      
      // Recurse into directories
      if (stats.isDirectory()) {
        await this.searchFilesRecursive(fullPath, pattern, contentSearch, results, maxResults);
      }
    }
  }

  async watchFile(args, connectionId) {
    const watchPath = this.resolvePath(args.path);
    const events = args.events || ['change'];
    const watcherId = uuidv4();
    
    try {
      const watcher = chokidar.watch(watchPath, {
        ignored: /(^|[\/\\])\../, // ignore dotfiles
        persistent: true
      });
      
      const connection = this.connections.get(connectionId);
      if (connection) {
        connection.watchers.add(watcherId);
      }
      
      this.fileWatchers.set(watcherId, watcher);
      
      // Set up event handlers
      events.forEach(event => {
        watcher.on(event, (path) => {
          const connection = this.connections.get(connectionId);
          if (connection && connection.ws.readyState === WebSocket.OPEN) {
            this.sendMessage(connection.ws, {
              jsonrpc: '2.0',
              method: 'notifications/file_changed',
              params: {
                watcherId,
                event,
                path: path.replace(this.basePath, '').replace(/^[\/\\]/, ''),
                timestamp: new Date().toISOString()
              }
            });
          }
        });
      });
      
      return {
        content: [
          {
            type: 'text',
            text: `File watcher started for: ${args.path}`
          }
        ],
        metadata: {
          watcherId,
          path: args.path,
          events,
          absolutePath: watchPath
        }
      };
    } catch (error) {
      throw new Error(`Failed to watch file: ${error.message}`);
    }
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
      case 'file://directory-tree':
        content = await this.getDirectoryTree();
        break;
      case 'file://disk-usage':
        content = await this.getDiskUsage();
        break;
      case 'file://recent-files':
        content = await this.getRecentFiles();
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
            text: JSON.stringify(content, null, 2)
          }
        ]
      }
    });
  }

  async getDirectoryTree() {
    const tree = await this.buildDirectoryTree(this.basePath);
    return {
      basePath: this.basePath,
      tree
    };
  }

  async buildDirectoryTree(dirPath, maxDepth = 5, currentDepth = 0) {
    if (currentDepth >= maxDepth) return null;
    
    try {
      const items = [];
      const entries = await fs.readdir(dirPath);
      
      for (const entry of entries.slice(0, 100)) { // Limit entries
        if (entry.startsWith('.')) continue;
        
        const fullPath = path.join(dirPath, entry);
        const stats = await fs.stat(fullPath);
        
        const item = {
          name: entry,
          type: stats.isDirectory() ? 'directory' : 'file',
          size: stats.size,
          modified: stats.mtime.toISOString()
        };
        
        if (stats.isDirectory() && currentDepth < maxDepth - 1) {
          item.children = await this.buildDirectoryTree(fullPath, maxDepth, currentDepth + 1);
        }
        
        items.push(item);
      }
      
      return items;
    } catch {
      return null;
    }
  }

  async getDiskUsage() {
    // Simple disk usage calculation
    const usage = await this.calculateDirectorySize(this.basePath);
    return {
      basePath: this.basePath,
      totalSize: usage.size,
      fileCount: usage.files,
      directoryCount: usage.directories,
      timestamp: new Date().toISOString()
    };
  }

  async calculateDirectorySize(dirPath) {
    let totalSize = 0;
    let fileCount = 0;
    let directoryCount = 0;
    
    try {
      const entries = await fs.readdir(dirPath);
      
      for (const entry of entries) {
        const fullPath = path.join(dirPath, entry);
        const stats = await fs.stat(fullPath);
        
        if (stats.isDirectory()) {
          directoryCount++;
          const subUsage = await this.calculateDirectorySize(fullPath);
          totalSize += subUsage.size;
          fileCount += subUsage.files;
          directoryCount += subUsage.directories;
        } else {
          fileCount++;
          totalSize += stats.size;
        }
      }
    } catch {
      // Ignore errors for inaccessible directories
    }
    
    return { size: totalSize, files: fileCount, directories: directoryCount };
  }

  async getRecentFiles() {
    const recentFiles = [];
    await this.findRecentFiles(this.basePath, recentFiles, 50);
    
    // Sort by modification time (most recent first)
    recentFiles.sort((a, b) => new Date(b.modified) - new Date(a.modified));
    
    return {
      basePath: this.basePath,
      files: recentFiles.slice(0, 20), // Return top 20
      timestamp: new Date().toISOString()
    };
  }

  async findRecentFiles(dirPath, results, maxFiles) {
    if (results.length >= maxFiles) return;
    
    try {
      const entries = await fs.readdir(dirPath);
      
      for (const entry of entries) {
        if (results.length >= maxFiles) break;
        if (entry.startsWith('.')) continue;
        
        const fullPath = path.join(dirPath, entry);
        const stats = await fs.stat(fullPath);
        
        if (stats.isFile()) {
          results.push({
            name: entry,
            path: path.relative(this.basePath, fullPath),
            size: stats.size,
            modified: stats.mtime.toISOString(),
            mimeType: mime.lookup(fullPath) || 'application/octet-stream'
          });
        } else if (stats.isDirectory()) {
          await this.findRecentFiles(fullPath, results, maxFiles);
        }
      }
    } catch {
      // Ignore errors for inaccessible directories
    }
  }

  resolvePath(relativePath) {
    // Ensure path is within base directory
    const resolved = path.resolve(this.basePath, relativePath);
    if (!resolved.startsWith(this.basePath)) {
      throw new Error('Path outside of allowed base directory');
    }
    return resolved;
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
    // Close all watchers
    this.fileWatchers.forEach(watcher => watcher.close());
    this.fileWatchers.clear();
    
    if (this.wss) {
      this.wss.close();
      console.log(`[${this.serverId}] Server stopped`);
    }
  }
}

// Start the server
const server = new FileToolsMCPServer(argv.port, argv.host, argv['base-path']);
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

module.exports = FileToolsMCPServer;
