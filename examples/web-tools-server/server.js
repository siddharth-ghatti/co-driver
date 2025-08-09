#!/usr/bin/env node

const WebSocket = require('ws');
const { v4: uuidv4 } = require('uuid');
const yargs = require('yargs');
const axios = require('axios');
const cheerio = require('cheerio');
const { URL } = require('url');
const validator = require('validator');

// Parse command line arguments
const argv = yargs
  .option('port', {
    alias: 'p',
    description: 'Port to listen on',
    type: 'number',
    default: 4002
  })
  .option('host', {
    alias: 'h',
    description: 'Host to bind to',
    type: 'string',
    default: '0.0.0.0'
  })
  .option('browser', {
    alias: 'b',
    description: 'Enable browser automation (requires Puppeteer)',
    type: 'boolean',
    default: false
  })
  .help()
  .argv;

class WebToolsMCPServer {
  constructor(port, host, enableBrowser = false) {
    this.port = port;
    this.host = host;
    this.enableBrowser = enableBrowser;
    this.serverId = process.env.MCP_SERVER_ID || `web-tools-${port}`;
    this.connections = new Map();
    this.browser = null;
    this.tools = this.initializeTools();
    this.resources = this.initializeResources();
    
    // Configure axios defaults
    this.httpClient = axios.create({
      timeout: 30000,
      maxRedirects: 5,
      headers: {
        'User-Agent': 'MCP-Web-Tools-Server/1.0.0'
      }
    });
  }

  initializeTools() {
    const tools = [
      {
        name: "fetch_url",
        description: "Fetch content from a URL using HTTP GET request",
        inputSchema: {
          type: "object",
          properties: {
            url: {
              type: "string",
              description: "URL to fetch"
            },
            headers: {
              type: "object",
              description: "Custom headers to include",
              additionalProperties: { type: "string" }
            },
            timeout: {
              type: "number",
              description: "Request timeout in milliseconds",
              default: 30000
            },
            follow_redirects: {
              type: "boolean",
              description: "Follow redirects",
              default: true
            }
          },
          required: ["url"]
        }
      },
      {
        name: "post_data",
        description: "Send HTTP POST request with data",
        inputSchema: {
          type: "object",
          properties: {
            url: {
              type: "string",
              description: "URL to post to"
            },
            data: {
              type: "object",
              description: "Data to send in request body"
            },
            headers: {
              type: "object",
              description: "Custom headers to include",
              additionalProperties: { type: "string" }
            },
            content_type: {
              type: "string",
              description: "Content type",
              enum: ["application/json", "application/x-www-form-urlencoded", "text/plain"],
              default: "application/json"
            }
          },
          required: ["url", "data"]
        }
      },
      {
        name: "scrape_page",
        description: "Scrape and extract content from a web page",
        inputSchema: {
          type: "object",
          properties: {
            url: {
              type: "string",
              description: "URL to scrape"
            },
            selector: {
              type: "string",
              description: "CSS selector to extract specific elements"
            },
            extract_links: {
              type: "boolean",
              description: "Extract all links from the page",
              default: false
            },
            extract_images: {
              type: "boolean",
              description: "Extract all images from the page",
              default: false
            },
            extract_text: {
              type: "boolean",
              description: "Extract clean text content",
              default: true
            },
            extract_metadata: {
              type: "boolean",
              description: "Extract page metadata (title, description, etc.)",
              default: true
            }
          },
          required: ["url"]
        }
      },
      {
        name: "validate_url",
        description: "Validate and analyze a URL",
        inputSchema: {
          type: "object",
          properties: {
            url: {
              type: "string",
              description: "URL to validate"
            },
            check_accessibility: {
              type: "boolean",
              description: "Check if URL is accessible",
              default: true
            }
          },
          required: ["url"]
        }
      },
      {
        name: "extract_links",
        description: "Extract all links from a web page",
        inputSchema: {
          type: "object",
          properties: {
            url: {
              type: "string",
              description: "URL to extract links from"
            },
            filter_domain: {
              type: "string",
              description: "Only return links from this domain"
            },
            internal_only: {
              type: "boolean",
              description: "Only return internal links",
              default: false
            }
          },
          required: ["url"]
        }
      },
      {
        name: "check_status",
        description: "Check HTTP status of multiple URLs",
        inputSchema: {
          type: "object",
          properties: {
            urls: {
              type: "array",
              items: { type: "string" },
              description: "URLs to check"
            },
            timeout: {
              type: "number",
              description: "Request timeout in milliseconds",
              default: 10000
            }
          },
          required: ["urls"]
        }
      },
      {
        name: "download_file",
        description: "Download a file from a URL",
        inputSchema: {
          type: "object",
          properties: {
            url: {
              type: "string",
              description: "URL of file to download"
            },
            max_size: {
              type: "number",
              description: "Maximum file size in bytes",
              default: 10485760
            },
            return_content: {
              type: "boolean",
              description: "Return file content in response",
              default: false
            }
          },
          required: ["url"]
        }
      },
      {
        name: "search_content",
        description: "Search for text content within a web page",
        inputSchema: {
          type: "object",
          properties: {
            url: {
              type: "string",
              description: "URL to search"
            },
            query: {
              type: "string",
              description: "Text to search for"
            },
            case_sensitive: {
              type: "boolean",
              description: "Case sensitive search",
              default: false
            },
            regex: {
              type: "boolean",
              description: "Treat query as regex pattern",
              default: false
            }
          },
          required: ["url", "query"]
        }
      },
      {
        name: "get_page_info",
        description: "Get comprehensive information about a web page",
        inputSchema: {
          type: "object",
          properties: {
            url: {
              type: "string",
              description: "URL to analyze"
            }
          },
          required: ["url"]
        }
      }
    ];

    // Add browser-based tools if enabled
    if (this.enableBrowser) {
      tools.push(
        {
          name: "screenshot_page",
          description: "Take a screenshot of a web page",
          inputSchema: {
            type: "object",
            properties: {
              url: {
                type: "string",
                description: "URL to screenshot"
              },
              width: {
                type: "number",
                description: "Viewport width",
                default: 1280
              },
              height: {
                type: "number",
                description: "Viewport height",
                default: 720
              },
              full_page: {
                type: "boolean",
                description: "Capture full page",
                default: false
              },
              wait_for: {
                type: "string",
                description: "CSS selector to wait for before screenshot"
              }
            },
            required: ["url"]
          }
        },
        {
          name: "interact_page",
          description: "Interact with a web page (click, type, etc.)",
          inputSchema: {
            type: "object",
            properties: {
              url: {
                type: "string",
                description: "URL to interact with"
              },
              actions: {
                type: "array",
                items: {
                  type: "object",
                  properties: {
                    type: {
                      type: "string",
                      enum: ["click", "type", "wait", "scroll"]
                    },
                    selector: { type: "string" },
                    text: { type: "string" },
                    delay: { type: "number" }
                  }
                },
                description: "Actions to perform on the page"
              }
            },
            required: ["url", "actions"]
          }
        }
      );
    }

    return tools;
  }

  initializeResources() {
    return [
      {
        uri: "web://request-history",
        name: "Request History",
        description: "History of HTTP requests made",
        mimeType: "application/json"
      },
      {
        uri: "web://cache-stats",
        name: "Cache Statistics",
        description: "HTTP cache statistics",
        mimeType: "application/json"
      },
      {
        uri: "web://browser-status",
        name: "Browser Status",
        description: "Browser automation status (if enabled)",
        mimeType: "application/json"
      }
    ];
  }

  async start() {
    // Initialize browser if enabled
    if (this.enableBrowser) {
      try {
        const puppeteer = require('puppeteer');
        this.browser = await puppeteer.launch({
          headless: true,
          args: ['--no-sandbox', '--disable-setuid-sandbox']
        });
        console.log(`[${this.serverId}] Browser automation enabled`);
      } catch (error) {
        console.warn(`[${this.serverId}] Failed to initialize browser:`, error.message);
        this.enableBrowser = false;
      }
    }

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
        requestHistory: []
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

    console.log(`[${this.serverId}] Web Tools MCP Server listening on ${this.host}:${this.port}`);
    
    this.setupHealthCheck();
  }

  setupHealthCheck() {
    const http = require('http');
    const healthPort = this.port + 1000;
    
    const healthServer = http.createServer(async (req, res) => {
      if (req.url === '/health') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          status: 'healthy',
          serverId: this.serverId,
          connections: this.connections.size,
          browserEnabled: this.enableBrowser,
          browserActive: this.browser !== null,
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
          description: 'Web Tools MCP Server - provides web scraping and HTTP utilities'
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
    const startTime = Date.now();
    
    let result;
    try {
      switch (name) {
        case 'fetch_url':
          result = await this.fetchUrl(args);
          break;
        case 'post_data':
          result = await this.postData(args);
          break;
        case 'scrape_page':
          result = await this.scrapePage(args);
          break;
        case 'validate_url':
          result = await this.validateUrl(args);
          break;
        case 'extract_links':
          result = await this.extractLinks(args);
          break;
        case 'check_status':
          result = await this.checkStatus(args);
          break;
        case 'download_file':
          result = await this.downloadFile(args);
          break;
        case 'search_content':
          result = await this.searchContent(args);
          break;
        case 'get_page_info':
          result = await this.getPageInfo(args);
          break;
        case 'screenshot_page':
          if (this.enableBrowser) {
            result = await this.screenshotPage(args);
          } else {
            throw new Error('Browser automation is not enabled');
          }
          break;
        case 'interact_page':
          if (this.enableBrowser) {
            result = await this.interactPage(args);
          } else {
            throw new Error('Browser automation is not enabled');
          }
          break;
        default:
          throw new Error(`Unknown tool: ${name}`);
      }

      // Log request to history
      const connection = this.connections.get(connectionId);
      if (connection) {
        connection.requestHistory.push({
          tool: name,
          url: args.url,
          timestamp: new Date().toISOString(),
          duration: Date.now() - startTime,
          success: true
        });
      }

    } catch (error) {
      // Log failed request to history
      const connection = this.connections.get(connectionId);
      if (connection) {
        connection.requestHistory.push({
          tool: name,
          url: args.url,
          timestamp: new Date().toISOString(),
          duration: Date.now() - startTime,
          success: false,
          error: error.message
        });
      }
      throw error;
    }

    this.sendMessage(ws, {
      jsonrpc: '2.0',
      id,
      result
    });
  }

  async fetchUrl(args) {
    const { url, headers = {}, timeout = 30000, follow_redirects = true } = args;
    
    try {
      const response = await this.httpClient.get(url, {
        headers,
        timeout,
        maxRedirects: follow_redirects ? 5 : 0,
        validateStatus: () => true // Don't throw on HTTP error status
      });

      return {
        content: [
          {
            type: 'text',
            text: response.data
          }
        ],
        metadata: {
          url,
          status: response.status,
          statusText: response.statusText,
          headers: response.headers,
          size: response.data.length,
          contentType: response.headers['content-type']
        }
      };
    } catch (error) {
      throw new Error(`Failed to fetch URL: ${error.message}`);
    }
  }

  async postData(args) {
    const { url, data, headers = {}, content_type = 'application/json' } = args;
    
    try {
      const config = {
        headers: {
          'Content-Type': content_type,
          ...headers
        }
      };

      let postData = data;
      if (content_type === 'application/x-www-form-urlencoded') {
        postData = new URLSearchParams(data).toString();
      } else if (content_type === 'application/json') {
        postData = JSON.stringify(data);
      }

      const response = await this.httpClient.post(url, postData, config);

      return {
        content: [
          {
            type: 'text',
            text: typeof response.data === 'string' ? response.data : JSON.stringify(response.data)
          }
        ],
        metadata: {
          url,
          status: response.status,
          statusText: response.statusText,
          headers: response.headers,
          contentType: response.headers['content-type']
        }
      };
    } catch (error) {
      throw new Error(`Failed to post data: ${error.message}`);
    }
  }

  async scrapePage(args) {
    const { 
      url, 
      selector, 
      extract_links = false, 
      extract_images = false, 
      extract_text = true, 
      extract_metadata = true 
    } = args;
    
    try {
      const response = await this.httpClient.get(url);
      const $ = cheerio.load(response.data);
      
      const result = {
        url,
        timestamp: new Date().toISOString()
      };

      if (extract_metadata) {
        result.metadata = {
          title: $('title').text().trim(),
          description: $('meta[name="description"]').attr('content') || '',
          keywords: $('meta[name="keywords"]').attr('content') || '',
          author: $('meta[name="author"]').attr('content') || '',
          canonical: $('link[rel="canonical"]').attr('href') || '',
          ogTitle: $('meta[property="og:title"]').attr('content') || '',
          ogDescription: $('meta[property="og:description"]').attr('content') || '',
          ogImage: $('meta[property="og:image"]').attr('content') || ''
        };
      }

      if (selector) {
        const elements = $(selector);
        result.selected_content = elements.map((i, el) => ({
          text: $(el).text().trim(),
          html: $(el).html(),
          attributes: el.attribs
        })).get();
      }

      if (extract_text) {
        // Remove script and style elements
        $('script, style').remove();
        result.text_content = $('body').text().replace(/\s+/g, ' ').trim();
      }

      if (extract_links) {
        result.links = $('a[href]').map((i, el) => ({
          text: $(el).text().trim(),
          href: $(el).attr('href'),
          title: $(el).attr('title') || ''
        })).get();
      }

      if (extract_images) {
        result.images = $('img[src]').map((i, el) => ({
          src: $(el).attr('src'),
          alt: $(el).attr('alt') || '',
          title: $(el).attr('title') || '',
          width: $(el).attr('width'),
          height: $(el).attr('height')
        })).get();
      }

      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(result, null, 2)
          }
        ],
        metadata: {
          url,
          elementCount: selector ? $(selector).length : null,
          textLength: result.text_content ? result.text_content.length : null,
          linkCount: result.links ? result.links.length : null,
          imageCount: result.images ? result.images.length : null
        }
      };
    } catch (error) {
      throw new Error(`Failed to scrape page: ${error.message}`);
    }
  }

  async validateUrl(args) {
    const { url, check_accessibility = true } = args;
    
    const validation = {
      url,
      isValid: validator.isURL(url),
      timestamp: new Date().toISOString()
    };

    if (validation.isValid) {
      try {
        const urlObj = new URL(url);
        validation.parsed = {
          protocol: urlObj.protocol,
          hostname: urlObj.hostname,
          port: urlObj.port,
          pathname: urlObj.pathname,
          search: urlObj.search,
          hash: urlObj.hash
        };

        if (check_accessibility) {
          try {
            const response = await this.httpClient.head(url, { timeout: 10000 });
            validation.accessibility = {
              accessible: true,
              status: response.status,
              headers: response.headers
            };
          } catch (error) {
            validation.accessibility = {
              accessible: false,
              error: error.message
            };
          }
        }
      } catch (error) {
        validation.parseError = error.message;
      }
    }

    return {
      content: [
        {
          type: 'text',
          text: JSON.stringify(validation, null, 2)
        }
      ],
      metadata: validation
    };
  }

  async extractLinks(args) {
    const { url, filter_domain, internal_only = false } = args;
    
    try {
      const response = await this.httpClient.get(url);
      const $ = cheerio.load(response.data);
      const baseUrl = new URL(url);
      
      const links = $('a[href]').map((i, el) => {
        const href = $(el).attr('href');
        const text = $(el).text().trim();
        
        try {
          const linkUrl = new URL(href, url);
          return {
            text,
            href: linkUrl.href,
            domain: linkUrl.hostname,
            isInternal: linkUrl.hostname === baseUrl.hostname,
            isExternal: linkUrl.hostname !== baseUrl.hostname
          };
        } catch {
          return {
            text,
            href,
            isInvalid: true
          };
        }
      }).get();

      let filteredLinks = links.filter(link => !link.isInvalid);

      if (internal_only) {
        filteredLinks = filteredLinks.filter(link => link.isInternal);
      }

      if (filter_domain) {
        filteredLinks = filteredLinks.filter(link => link.domain === filter_domain);
      }

      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(filteredLinks, null, 2)
          }
        ],
        metadata: {
          url,
          totalLinks: links.length,
          filteredLinks: filteredLinks.length,
          filterDomain: filter_domain,
          internalOnly: internal_only
        }
      };
    } catch (error) {
      throw new Error(`Failed to extract links: ${error.message}`);
    }
  }

  async checkStatus(args) {
    const { urls, timeout = 10000 } = args;
    
    const results = await Promise.allSettled(
      urls.map(async (url) => {
        try {
          const response = await this.httpClient.head(url, { timeout });
          return {
            url,
            status: response.status,
            statusText: response.statusText,
            accessible: true,
            responseTime: response.headers['x-response-time'] || null
          };
        } catch (error) {
          return {
            url,
            status: error.response ? error.response.status : null,
            accessible: false,
            error: error.message
          };
        }
      })
    );

    const statusResults = results.map((result, index) => ({
      url: urls[index],
      ...(result.status === 'fulfilled' ? result.value : { error: result.reason.message, accessible: false })
    }));

    return {
      content: [
        {
          type: 'text',
          text: JSON.stringify(statusResults, null, 2)
        }
      ],
      metadata: {
        totalUrls: urls.length,
        accessible: statusResults.filter(r => r.accessible).length,
        failed: statusResults.filter(r => !r.accessible).length
      }
    };
  }

  async downloadFile(args) {
    const { url, max_size = 10485760, return_content = false } = args;
    
    try {
      const response = await this.httpClient.get(url, {
        responseType: 'arraybuffer',
        maxContentLength: max_size,
        maxBodyLength: max_size
      });

      const buffer = Buffer.from(response.data);
      const contentType = response.headers['content-type'] || 'application/octet-stream';
      
      const result = {
        content: [
          {
            type: 'text',
            text: `File downloaded successfully: ${buffer.length} bytes`
          }
        ],
        metadata: {
          url,
          size: buffer.length,
          contentType,
          filename: this.extractFilenameFromUrl(url) || 'download',
          headers: response.headers
        }
      };

      if (return_content) {
        result.fileContent = buffer.toString('base64');
        result.encoding = 'base64';
      }

      return result;
    } catch (error) {
      throw new Error(`Failed to download file: ${error.message}`);
    }
  }

  extractFilenameFromUrl(url) {
    try {
      const urlObj = new URL(url);
      const pathname = urlObj.pathname;
      return pathname.split('/').pop() || null;
    } catch {
      return null;
    }
  }

  async searchContent(args) {
    const { url, query, case_sensitive = false, regex = false } = args;
    
    try {
      const response = await this.httpClient.get(url);
      const $ = cheerio.load(response.data);
      
      // Remove script and style elements
      $('script, style').remove();
      const text = $('body').text();
      
      let searchPattern;
      if (regex) {
        const flags = case_sensitive ? 'g' : 'gi';
        searchPattern = new RegExp(query, flags);
      } else {
        const escapedQuery = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        const flags = case_sensitive ? 'g' : 'gi';
        searchPattern = new RegExp(escapedQuery, flags);
      }

      const matches = [];
      let match;
      while ((match = searchPattern.exec(text)) !== null) {
        const start = Math.max(0, match.index - 50);
        const end = Math.min(text.length, match.index + match[0].length + 50);
        const context = text.substring(start, end);
        
        matches.push({
          match: match[0],
          index: match.index,
          context: context.replace(/\s+/g, ' ').trim()
        });
        
        if (!regex) break; // For non-regex, just find first match
      }

      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify({
              query,
              url,
              matches,
              totalMatches: matches.length,
              searchType: regex ? 'regex' : 'text',
              caseSensitive: case_sensitive
            }, null, 2)
          }
        ],
        metadata: {
          url,
          query,
          matchCount: matches.length,
          regex,
          caseSensitive: case_sensitive
        }
      };
    } catch (error) {
      throw new Error(`Failed to search content: ${error.message}`);
    }
  }

  async getPageInfo(args) {
    const { url } = args;
    
    try {
      const response = await this.httpClient.get(url);
      const $ = cheerio.load(response.data);
      
      // Extract comprehensive page information
      const info = {
        url,
        timestamp: new Date().toISOString(),
        
        // Basic info
        title: $('title').text().trim(),
        description: $('meta[name="description"]').attr('content') || '',
        language: $('html').attr('lang') || $('meta[http-equiv="content-language"]').attr('content') || '',
        
        // Meta tags
        meta: {
          keywords: $('meta[name="keywords"]').attr('content') || '',
          author: $('meta[name="author"]').attr('content') || '',
          viewport: $('meta[name="viewport"]').attr('content') || '',
          robots: $('meta[name="robots"]').attr('content') || '',
          canonical: $('link[rel="canonical"]').attr('href') || ''
        },
        
        // Open Graph
        openGraph: {
          title: $('meta[property="og:title"]').attr('content') || '',
          description: $('meta[property="og:description"]').attr('content') || '',
          image: $('meta[property="og:image"]').attr('content') || '',
          url: $('meta[property="og:url"]').attr('content') || '',
          type: $('meta[property="og:type"]').attr('content') || '',
          siteName: $('meta[property="og:site_name"]').attr('content') || ''
        },
        
        // Twitter Card
        twitter: {
          card: $('meta[name="twitter:card"]').attr('content') || '',
          site: $('meta[name="twitter:site"]').attr('content') || '',
          creator: $('meta[name="twitter:creator"]').attr('content') || '',
          title: $('meta[name="twitter:title"]').attr('content') || '',
          description: $('meta[name="twitter:description"]').attr('content') || '',
          image: $('meta[name="twitter:image"]').attr('content') || ''
        },
        
        // Content analysis
        content: {
          headings: {
            h1: $('h1').length,
            h2: $('h2').length,
            h3: $('h3').length,
            h4: $('h4').length,
            h5: $('h5').length,
            h6: $('h6').length
          },
          links: {
            internal: 0,
            external: 0,
            total: $('a[href]').length
          },
          images: $('img').length,
          forms: $('form').length,
          scripts: $('script').length,
          stylesheets: $('link[rel="stylesheet"]').length
        },
        
        // Response info
        response: {
          status: response.status,
          headers: response.headers,
          size: response.data.length
        }
      };

      // Analyze links
      const baseUrl = new URL(url);
      $('a[href]').each((i, el) => {
        const href = $(el).attr('href');
        try {
          const linkUrl = new URL(href, url);
          if (linkUrl.hostname === baseUrl.hostname) {
            info.content.links.internal++;
          } else {
            info.content.links.external++;
          }
        } catch {
          // Invalid URL
        }
      });

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
      throw new Error(`Failed to get page info: ${error.message}`);
    }
  }

  async screenshotPage(args) {
    if (!this.browser) {
      throw new Error('Browser is not available');
    }

    const { url, width = 1280, height = 720, full_page = false, wait_for } = args;
    
    try {
      const page = await this.browser.newPage();
      await page.setViewport({ width, height });
      
      await page.goto(url, { waitUntil: 'networkidle2' });
      
      if (wait_for) {
        await page.waitForSelector(wait_for, { timeout: 30000 });
      }
      
      const screenshot = await page.screenshot({
        fullPage: full_page,
        encoding: 'base64'
      });
      
      await page.close();
      
      return {
        content: [
          {
            type: 'text',
            text: 'Screenshot captured successfully'
          }
        ],
        metadata: {
          url,
          width,
          height,
          fullPage: full_page,
          size: screenshot.length
        },
        screenshot: screenshot,
        encoding: 'base64'
      };
    } catch (error) {
      throw new Error(`Failed to take screenshot: ${error.message}`);
    }
  }

  async interactPage(args) {
    if (!this.browser) {
      throw new Error('Browser is not available');
    }

    const { url, actions } = args;
    
    try {
      const page = await this.browser.newPage();
      await page.goto(url, { waitUntil: 'networkidle2' });
      
      const results = [];
      
      for (const action of actions) {
        const result = { action: action.type };
        
        try {
          switch (action.type) {
            case 'click':
              await page.click(action.selector);
              result.success = true;
              break;
              
            case 'type':
              await page.type(action.selector, action.text);
              result.success = true;
              break;
              
            case 'wait':
              if (action.selector) {
                await page.waitForSelector(action.selector, { timeout: 30000 });
              } else {
                await page.waitForTimeout(action.delay || 1000);
              }
              result.success = true;
              break;
              
            case 'scroll':
              await page.evaluate((selector) => {
                const element = document.querySelector(selector);
                if (element) {
                  element.scrollIntoView();
                }
              }, action.selector);
              result.success = true;
              break;
              
            default:
              result.success = false;
              result.error = `Unknown action type: ${action.type}`;
          }
        } catch (error) {
          result.success = false;
          result.error = error.message;
        }
        
        results.push(result);
      }
      
      await page.close();
      
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(results, null, 2)
          }
        ],
        metadata: {
          url,
          actionsPerformed: results.length,
          successful: results.filter(r => r.success).length,
          failed: results.filter(r => !r.success).length
        }
      };
    } catch (error) {
      throw new Error(`Failed to interact with page: ${error.message}`);
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
      case 'web://request-history':
        content = this.getRequestHistory();
        break;
      case 'web://cache-stats':
        content = this.getCacheStats();
        break;
      case 'web://browser-status':
        content = this.getBrowserStatus();
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

  getRequestHistory() {
    const allHistory = [];
    this.connections.forEach((connection, connectionId) => {
      connection.requestHistory.forEach(request => {
        allHistory.push({
          connectionId,
          ...request
        });
      });
    });
    
    return {
      totalRequests: allHistory.length,
      requests: allHistory.sort((a, b) => new Date(b.timestamp) - new Date(a.timestamp)).slice(0, 100)
    };
  }

  getCacheStats() {
    return {
      enabled: false,
      message: 'HTTP caching not implemented in this version'
    };
  }

  getBrowserStatus() {
    return {
      enabled: this.enableBrowser,
      active: this.browser !== null,
      version: this.enableBrowser ? 'Puppeteer' : null,
      capabilities: this.enableBrowser ? ['screenshot', 'interaction'] : []
    };
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

  async stop() {
    if (this.browser) {
      await this.browser.close();
      this.browser = null;
    }
    
    if (this.wss) {
      this.wss.close();
      console.log(`[${this.serverId}] Server stopped`);
    }
  }
}

// Start the server
const server = new WebToolsMCPServer(argv.port, argv.host, argv.browser);
server.start();

// Graceful shutdown
process.on('SIGINT', async () => {
  console.log('\nReceived SIGINT, shutting down gracefully...');
  await server.stop();
  process.exit(0);
});

process.on('SIGTERM', async () => {
  console.log('\nReceived SIGTERM, shutting down gracefully...');
  await server.stop();
  process.exit(0);
});

module.exports = WebToolsMCPServer;
