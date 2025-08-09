#!/usr/bin/env node

const WebSocket = require('ws');
const http = require('http');
const { performance } = require('perf_hooks');

// Test configuration
const GATEWAY_URL = process.env.GATEWAY_URL || 'ws://localhost:8080/mcp';
const GATEWAY_HTTP = process.env.GATEWAY_HTTP || 'http://localhost:8080';
const TIMEOUT = 30000; // 30 seconds

class MCPDeploymentTester {
  constructor() {
    this.testResults = [];
    this.startTime = performance.now();
  }

  log(message, type = 'info') {
    const timestamp = new Date().toISOString();
    const prefix = type.toUpperCase().padEnd(5);
    console.log(`[${timestamp}] ${prefix} ${message}`);
  }

  async runAllTests() {
    this.log('Starting MCP Gateway and Servers deployment test...', 'info');
    this.log(`Gateway URL: ${GATEWAY_URL}`, 'info');
    
    try {
      // Test 1: Gateway Health Check
      await this.testGatewayHealth();
      
      // Test 2: WebSocket Connection
      await this.testWebSocketConnection();
      
      // Test 3: MCP Protocol Basic Flow
      await this.testMCPProtocol();
      
      // Test 4: Load Balancing
      await this.testLoadBalancing();
      
      // Test 5: Server Health Checks
      await this.testServerHealth();
      
      // Test 6: Metrics Endpoint
      await this.testMetrics();
      
      // Generate Report
      this.generateReport();
      
    } catch (error) {
      this.log(`Test suite failed: ${error.message}`, 'error');
      process.exit(1);
    }
  }

  async testGatewayHealth() {
    this.log('Testing gateway health endpoint...', 'test');
    
    try {
      const response = await this.httpRequest(`${GATEWAY_HTTP}/health`);
      
      if (response.status === 'healthy' || response.status === 200) {
        this.addResult('Gateway Health', 'PASS', 'Gateway is healthy');
      } else {
        this.addResult('Gateway Health', 'FAIL', `Unexpected health status: ${response.status}`);
      }
    } catch (error) {
      this.addResult('Gateway Health', 'FAIL', error.message);
    }
  }

  async testWebSocketConnection() {
    this.log('Testing WebSocket connection...', 'test');
    
    return new Promise((resolve) => {
      const ws = new WebSocket(GATEWAY_URL);
      let connected = false;
      
      const timeout = setTimeout(() => {
        if (!connected) {
          this.addResult('WebSocket Connection', 'FAIL', 'Connection timeout');
          ws.terminate();
          resolve();
        }
      }, TIMEOUT);
      
      ws.on('open', () => {
        connected = true;
        clearTimeout(timeout);
        this.addResult('WebSocket Connection', 'PASS', 'Successfully connected to gateway');
        ws.close();
        resolve();
      });
      
      ws.on('error', (error) => {
        connected = true;
        clearTimeout(timeout);
        this.addResult('WebSocket Connection', 'FAIL', error.message);
        resolve();
      });
    });
  }

  async testMCPProtocol() {
    this.log('Testing MCP protocol flow...', 'test');
    
    return new Promise((resolve) => {
      const ws = new WebSocket(GATEWAY_URL);
      let messageCount = 0;
      const expectedMessages = 3; // initialize, ping, tools/list
      
      const timeout = setTimeout(() => {
        this.addResult('MCP Protocol', 'FAIL', 'Protocol test timeout');
        ws.terminate();
        resolve();
      }, TIMEOUT);
      
      ws.on('open', () => {
        // Test 1: Initialize
        ws.send(JSON.stringify({
          jsonrpc: '2.0',
          id: 1,
          method: 'initialize',
          params: {
            protocolVersion: '2024-11-05',
            capabilities: {},
            clientInfo: {
              name: 'deployment-tester',
              version: '1.0.0'
            }
          }
        }));
      });
      
      ws.on('message', (data) => {
        try {
          const message = JSON.parse(data.toString());
          messageCount++;
          
          if (message.id === 1 && message.result) {
            // Initialize successful, test ping
            ws.send(JSON.stringify({
              jsonrpc: '2.0',
              id: 2,
              method: 'ping'
            }));
          } else if (message.id === 2 && message.result !== undefined) {
            // Ping successful, test tools list
            ws.send(JSON.stringify({
              jsonrpc: '2.0',
              id: 3,
              method: 'tools/list'
            }));
          } else if (message.id === 3 && message.result) {
            // All tests passed
            clearTimeout(timeout);
            this.addResult('MCP Protocol', 'PASS', `Successfully completed MCP handshake and basic operations`);
            ws.close();
            resolve();
          } else if (message.error) {
            clearTimeout(timeout);
            this.addResult('MCP Protocol', 'FAIL', `MCP error: ${message.error.message}`);
            ws.close();
            resolve();
          }
        } catch (error) {
          clearTimeout(timeout);
          this.addResult('MCP Protocol', 'FAIL', `Message parse error: ${error.message}`);
          ws.close();
          resolve();
        }
      });
      
      ws.on('error', (error) => {
        clearTimeout(timeout);
        this.addResult('MCP Protocol', 'FAIL', error.message);
        resolve();
      });
    });
  }

  async testLoadBalancing() {
    this.log('Testing load balancing...', 'test');
    
    const serverIds = new Set();
    const testConnections = 5;
    const promises = [];
    
    for (let i = 0; i < testConnections; i++) {
      promises.push(this.getServerInfo());
    }
    
    try {
      const results = await Promise.all(promises);
      results.forEach(serverId => {
        if (serverId) serverIds.add(serverId);
      });
      
      if (serverIds.size > 1) {
        this.addResult('Load Balancing', 'PASS', `Requests distributed across ${serverIds.size} servers: ${Array.from(serverIds).join(', ')}`);
      } else if (serverIds.size === 1) {
        this.addResult('Load Balancing', 'WARN', `All requests went to single server: ${Array.from(serverIds)[0]}`);
      } else {
        this.addResult('Load Balancing', 'FAIL', 'No valid server responses received');
      }
    } catch (error) {
      this.addResult('Load Balancing', 'FAIL', error.message);
    }
  }

  async getServerInfo() {
    return new Promise((resolve) => {
      const ws = new WebSocket(GATEWAY_URL);
      
      const timeout = setTimeout(() => {
        ws.terminate();
        resolve(null);
      }, 10000);
      
      ws.on('open', () => {
        ws.send(JSON.stringify({
          jsonrpc: '2.0',
          id: 1,
          method: 'initialize',
          params: {
            protocolVersion: '2024-11-05',
            capabilities: {},
            clientInfo: { name: 'load-test', version: '1.0.0' }
          }
        }));
      });
      
      ws.on('message', (data) => {
        try {
          const message = JSON.parse(data.toString());
          if (message.result && message.result.serverInfo) {
            clearTimeout(timeout);
            ws.close();
            resolve(message.result.serverInfo.name);
          }
        } catch (error) {
          clearTimeout(timeout);
          ws.close();
          resolve(null);
        }
      });
      
      ws.on('error', () => {
        clearTimeout(timeout);
        resolve(null);
      });
    });
  }

  async testServerHealth() {
    this.log('Testing individual server health...', 'test');
    
    const serverPorts = [4001, 4002, 5001]; // health check ports
    const healthResults = [];
    
    for (const port of serverPorts) {
      try {
        const response = await this.httpRequest(`http://localhost:${port}/health`);
        healthResults.push({
          port,
          status: response.status || 'unknown',
          serverId: response.serverId || response.server_id || 'unknown'
        });
      } catch (error) {
        healthResults.push({
          port,
          status: 'error',
          error: error.message
        });
      }
    }
    
    const healthyServers = healthResults.filter(r => r.status === 'healthy').length;
    const totalServers = healthResults.length;
    
    if (healthyServers > 0) {
      this.addResult('Server Health', 'PASS', `${healthyServers}/${totalServers} servers healthy`);
      healthResults.forEach(result => {
        if (result.status === 'healthy') {
          this.log(`  ✓ ${result.serverId} (port ${result.port}): healthy`, 'info');
        } else {
          this.log(`  ✗ Port ${result.port}: ${result.error || result.status}`, 'warn');
        }
      });
    } else {
      this.addResult('Server Health', 'FAIL', 'No healthy servers found');
    }
  }

  async testMetrics() {
    this.log('Testing metrics endpoint...', 'test');
    
    try {
      const response = await this.httpRequest(`${GATEWAY_HTTP}/metrics`, { raw: true });
      
      if (typeof response === 'string' && response.includes('mcp_gateway')) {
        const metricLines = response.split('\n').filter(line => 
          line.startsWith('mcp_gateway') && !line.startsWith('#')
        );
        this.addResult('Metrics', 'PASS', `Found ${metricLines.length} gateway metrics`);
      } else {
        this.addResult('Metrics', 'FAIL', 'No gateway metrics found');
      }
    } catch (error) {
      this.addResult('Metrics', 'FAIL', error.message);
    }
  }

  async httpRequest(url, options = {}) {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('HTTP request timeout'));
      }, 10000);
      
      http.get(url, (res) => {
        clearTimeout(timeout);
        let data = '';
        
        res.on('data', (chunk) => {
          data += chunk;
        });
        
        res.on('end', () => {
          try {
            if (options.raw) {
              resolve(data);
            } else {
              resolve(JSON.parse(data));
            }
          } catch (error) {
            // If JSON parse fails, return raw data
            resolve(data);
          }
        });
      }).on('error', (error) => {
        clearTimeout(timeout);
        reject(error);
      });
    });
  }

  addResult(test, status, message) {
    this.testResults.push({ test, status, message });
    
    const statusSymbol = {
      'PASS': '✓',
      'FAIL': '✗',
      'WARN': '⚠'
    }[status] || '?';
    
    this.log(`${statusSymbol} ${test}: ${message}`, status.toLowerCase());
  }

  generateReport() {
    const endTime = performance.now();
    const duration = ((endTime - this.startTime) / 1000).toFixed(2);
    
    this.log('', 'info');
    this.log('='.repeat(60), 'info');
    this.log('DEPLOYMENT TEST REPORT', 'info');
    this.log('='.repeat(60), 'info');
    
    const passed = this.testResults.filter(r => r.status === 'PASS').length;
    const failed = this.testResults.filter(r => r.status === 'FAIL').length;
    const warnings = this.testResults.filter(r => r.status === 'WARN').length;
    
    this.log(`Total Tests: ${this.testResults.length}`, 'info');
    this.log(`Passed: ${passed}`, 'info');
    this.log(`Failed: ${failed}`, 'info');
    this.log(`Warnings: ${warnings}`, 'info');
    this.log(`Duration: ${duration}s`, 'info');
    this.log('', 'info');
    
    // Detailed results
    this.testResults.forEach(result => {
      const statusSymbol = {
        'PASS': '✓',
        'FAIL': '✗',
        'WARN': '⚠'
      }[result.status] || '?';
      
      this.log(`${statusSymbol} ${result.test.padEnd(20)} ${result.message}`, 'info');
    });
    
    this.log('', 'info');
    
    // Overall status
    if (failed === 0) {
      this.log('🎉 DEPLOYMENT TEST SUCCESSFUL!', 'info');
      if (warnings > 0) {
        this.log(`⚠️  Note: ${warnings} warnings detected`, 'warn');
      }
      process.exit(0);
    } else {
      this.log('❌ DEPLOYMENT TEST FAILED!', 'error');
      this.log(`${failed} critical issues detected`, 'error');
      process.exit(1);
    }
  }
}

// Command line interface
if (require.main === module) {
  const args = process.argv.slice(2);
  
  if (args.includes('--help') || args.includes('-h')) {
    console.log(`
MCP Gateway Deployment Tester

Usage: node test-deployment.js [options]

Options:
  --help, -h        Show this help message
  --gateway-url     WebSocket URL for the gateway (default: ws://localhost:8080/mcp)
  --gateway-http    HTTP URL for the gateway (default: http://localhost:8080)

Environment Variables:
  GATEWAY_URL       WebSocket URL for the gateway
  GATEWAY_HTTP      HTTP URL for the gateway

Examples:
  # Test local deployment
  node test-deployment.js

  # Test remote deployment
  GATEWAY_URL=ws://my-gateway.com/mcp GATEWAY_HTTP=http://my-gateway.com node test-deployment.js
  
  # Test with custom URLs
  node test-deployment.js --gateway-url ws://custom:8080/mcp --gateway-http http://custom:8080
    `);
    process.exit(0);
  }
  
  // Parse command line arguments
  const gatewayUrlIndex = args.indexOf('--gateway-url');
  if (gatewayUrlIndex !== -1 && args[gatewayUrlIndex + 1]) {
    process.env.GATEWAY_URL = args[gatewayUrlIndex + 1];
  }
  
  const gatewayHttpIndex = args.indexOf('--gateway-http');
  if (gatewayHttpIndex !== -1 && args[gatewayHttpIndex + 1]) {
    process.env.GATEWAY_HTTP = args[gatewayHttpIndex + 1];
  }
  
  // Run tests
  const tester = new MCPDeploymentTester();
  tester.runAllTests().catch(error => {
    console.error('Test runner error:', error);
    process.exit(1);
  });
}

module.exports = MCPDeploymentTester;
