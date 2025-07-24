#!/usr/bin/env node

/**
 * MCP Gateway WebSocket Test Script
 *
 * This script tests the WebSocket functionality of the MCP Gateway
 * by establishing a WebSocket connection and sending MCP protocol messages.
 */

const WebSocket = require('ws');
const readline = require('readline');

// Configuration
const GATEWAY_WS_URL = 'ws://localhost:8080/mcp';
const TEST_TIMEOUT = 30000; // 30 seconds

// Colors for console output
const colors = {
    reset: '\x1b[0m',
    bright: '\x1b[1m',
    red: '\x1b[31m',
    green: '\x1b[32m',
    yellow: '\x1b[33m',
    blue: '\x1b[34m',
    magenta: '\x1b[35m',
    cyan: '\x1b[36m'
};

// Utility functions
function log(message, color = colors.reset) {
    console.log(`${color}${message}${colors.reset}`);
}

function logInfo(message) {
    log(`ℹ️  ${message}`, colors.blue);
}

function logSuccess(message) {
    log(`✅ ${message}`, colors.green);
}

function logWarning(message) {
    log(`⚠️  ${message}`, colors.yellow);
}

function logError(message) {
    log(`❌ ${message}`, colors.red);
}

// MCP Protocol message templates
const mcpMessages = {
    ping: {
        jsonrpc: "2.0",
        id: 1,
        method: "ping"
    },

    initialize: {
        jsonrpc: "2.0",
        id: 2,
        method: "initialize",
        params: {
            protocolVersion: "2024-11-05",
            capabilities: {
                experimental: {},
                sampling: {}
            },
            clientInfo: {
                name: "mcp-gateway-test",
                version: "1.0.0"
            }
        }
    },

    toolsList: {
        jsonrpc: "2.0",
        id: 3,
        method: "tools/list"
    },

    resourcesList: {
        jsonrpc: "2.0",
        id: 4,
        method: "resources/list"
    },

    promptsList: {
        jsonrpc: "2.0",
        id: 5,
        method: "prompts/list"
    }
};

class MCPWebSocketTester {
    constructor(url) {
        this.url = url;
        this.ws = null;
        this.messageId = 100;
        this.pendingRequests = new Map();
        this.connected = false;
        this.initialized = false;
    }

    async connect() {
        return new Promise((resolve, reject) => {
            logInfo(`Connecting to MCP Gateway at ${this.url}`);

            this.ws = new WebSocket(this.url);

            const timeout = setTimeout(() => {
                reject(new Error('Connection timeout'));
            }, 5000);

            this.ws.on('open', () => {
                clearTimeout(timeout);
                this.connected = true;
                logSuccess('WebSocket connection established');
                this.setupEventHandlers();
                resolve();
            });

            this.ws.on('error', (error) => {
                clearTimeout(timeout);
                reject(error);
            });
        });
    }

    setupEventHandlers() {
        this.ws.on('message', (data) => {
            try {
                const message = JSON.parse(data.toString());
                this.handleMessage(message);
            } catch (error) {
                logError(`Failed to parse message: ${error.message}`);
                logError(`Raw message: ${data.toString()}`);
            }
        });

        this.ws.on('close', (code, reason) => {
            this.connected = false;
            logWarning(`WebSocket connection closed: ${code} ${reason}`);
        });

        this.ws.on('error', (error) => {
            logError(`WebSocket error: ${error.message}`);
        });
    }

    handleMessage(message) {
        logInfo(`Received message: ${JSON.stringify(message, null, 2)}`);

        if (message.id && this.pendingRequests.has(message.id)) {
            const { resolve } = this.pendingRequests.get(message.id);
            this.pendingRequests.delete(message.id);
            resolve(message);
        }
    }

    async sendMessage(message) {
        if (!this.connected) {
            throw new Error('WebSocket not connected');
        }

        return new Promise((resolve, reject) => {
            const messageStr = JSON.stringify(message);
            logInfo(`Sending message: ${messageStr}`);

            if (message.id) {
                const timeout = setTimeout(() => {
                    this.pendingRequests.delete(message.id);
                    reject(new Error(`Request timeout for message ID ${message.id}`));
                }, 10000);

                this.pendingRequests.set(message.id, {
                    resolve: (response) => {
                        clearTimeout(timeout);
                        resolve(response);
                    },
                    reject
                });
            }

            this.ws.send(messageStr, (error) => {
                if (error) {
                    if (message.id) {
                        this.pendingRequests.delete(message.id);
                    }
                    reject(error);
                } else if (!message.id) {
                    // For notifications (no response expected)
                    resolve();
                }
            });
        });
    }

    async runTests() {
        try {
            logInfo('Starting MCP WebSocket tests...\n');

            // Test 1: Basic ping
            logInfo('Test 1: Ping test');
            try {
                const pingResponse = await this.sendMessage(mcpMessages.ping);
                if (pingResponse.result !== undefined) {
                    logSuccess('Ping test passed');
                } else if (pingResponse.error) {
                    logWarning(`Ping failed with error: ${pingResponse.error.message}`);
                } else {
                    logWarning('Ping response format unexpected');
                }
            } catch (error) {
                logError(`Ping test failed: ${error.message}`);
            }

            await this.delay(1000);

            // Test 2: Initialize
            logInfo('Test 2: Initialize protocol');
            try {
                const initResponse = await this.sendMessage(mcpMessages.initialize);
                if (initResponse.result) {
                    logSuccess('Initialize successful');
                    this.initialized = true;
                    logInfo(`Server info: ${JSON.stringify(initResponse.result.serverInfo, null, 2)}`);
                } else if (initResponse.error) {
                    logWarning(`Initialize failed: ${initResponse.error.message}`);
                } else {
                    logWarning('Initialize response format unexpected');
                }
            } catch (error) {
                logError(`Initialize failed: ${error.message}`);
            }

            await this.delay(1000);

            // Test 3: Tools list
            logInfo('Test 3: List tools');
            try {
                const toolsResponse = await this.sendMessage(mcpMessages.toolsList);
                if (toolsResponse.result) {
                    logSuccess(`Tools list retrieved: ${toolsResponse.result.tools?.length || 0} tools`);
                } else if (toolsResponse.error) {
                    logWarning(`Tools list failed: ${toolsResponse.error.message}`);
                } else {
                    logWarning('Tools list response format unexpected');
                }
            } catch (error) {
                logError(`Tools list failed: ${error.message}`);
            }

            await this.delay(1000);

            // Test 4: Resources list
            logInfo('Test 4: List resources');
            try {
                const resourcesResponse = await this.sendMessage(mcpMessages.resourcesList);
                if (resourcesResponse.result) {
                    logSuccess(`Resources list retrieved: ${resourcesResponse.result.resources?.length || 0} resources`);
                } else if (resourcesResponse.error) {
                    logWarning(`Resources list failed: ${resourcesResponse.error.message}`);
                } else {
                    logWarning('Resources list response format unexpected');
                }
            } catch (error) {
                logError(`Resources list failed: ${error.message}`);
            }

            await this.delay(1000);

            // Test 5: Prompts list
            logInfo('Test 5: List prompts');
            try {
                const promptsResponse = await this.sendMessage(mcpMessages.promptsList);
                if (promptsResponse.result) {
                    logSuccess(`Prompts list retrieved: ${promptsResponse.result.prompts?.length || 0} prompts`);
                } else if (promptsResponse.error) {
                    logWarning(`Prompts list failed: ${promptsResponse.error.message}`);
                } else {
                    logWarning('Prompts list response format unexpected');
                }
            } catch (error) {
                logError(`Prompts list failed: ${error.message}`);
            }

            await this.delay(1000);

            // Test 6: Invalid method
            logInfo('Test 6: Invalid method (should return error)');
            try {
                const invalidResponse = await this.sendMessage({
                    jsonrpc: "2.0",
                    id: 99,
                    method: "invalid/method"
                });
                if (invalidResponse.error) {
                    logSuccess(`Invalid method correctly returned error: ${invalidResponse.error.message}`);
                } else {
                    logWarning('Invalid method should have returned an error');
                }
            } catch (error) {
                logError(`Invalid method test failed: ${error.message}`);
            }

            logSuccess('\n🎉 All WebSocket tests completed!');

        } catch (error) {
            logError(`Test execution failed: ${error.message}`);
        }
    }

    delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    close() {
        if (this.ws) {
            this.ws.close();
        }
    }
}

// Interactive mode
async function runInteractiveMode(tester) {
    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });

    logInfo('\n🎮 Interactive Mode');
    logInfo('Type MCP JSON messages or use these shortcuts:');
    logInfo('  ping         - Send a ping');
    logInfo('  init         - Send initialize');
    logInfo('  tools        - List tools');
    logInfo('  resources    - List resources');
    logInfo('  prompts      - List prompts');
    logInfo('  quit         - Exit interactive mode');
    logInfo('');

    const askQuestion = (question) => {
        return new Promise((resolve) => {
            rl.question(question, resolve);
        });
    };

    while (true) {
        try {
            const input = await askQuestion('mcp> ');

            if (input.trim() === 'quit') {
                break;
            }

            let message;

            switch (input.trim()) {
                case 'ping':
                    message = mcpMessages.ping;
                    break;
                case 'init':
                    message = mcpMessages.initialize;
                    break;
                case 'tools':
                    message = mcpMessages.toolsList;
                    break;
                case 'resources':
                    message = mcpMessages.resourcesList;
                    break;
                case 'prompts':
                    message = mcpMessages.promptsList;
                    break;
                default:
                    try {
                        message = JSON.parse(input);
                    } catch (error) {
                        logError('Invalid JSON. Please enter a valid MCP message or use shortcuts.');
                        continue;
                    }
            }

            if (message.id === undefined) {
                message.id = ++tester.messageId;
            }

            const response = await tester.sendMessage(message);
            console.log(JSON.stringify(response, null, 2));

        } catch (error) {
            logError(`Error: ${error.message}`);
        }
    }

    rl.close();
}

// Main execution
async function main() {
    const args = process.argv.slice(2);
    const interactive = args.includes('--interactive') || args.includes('-i');
    const url = args.find(arg => arg.startsWith('--url='))?.split('=')[1] || GATEWAY_WS_URL;

    console.log(`${colors.bright}🔌 MCP Gateway WebSocket Tester${colors.reset}`);
    console.log('=====================================\n');

    const tester = new MCPWebSocketTester(url);

    try {
        await tester.connect();

        if (interactive) {
            await runInteractiveMode(tester);
        } else {
            await tester.runTests();
        }

    } catch (error) {
        logError(`Connection failed: ${error.message}`);
        logInfo('Make sure the MCP Gateway is running at the specified URL');
        process.exit(1);
    } finally {
        tester.close();
    }
}

// Handle process termination
process.on('SIGINT', () => {
    logInfo('\nShutting down...');
    process.exit(0);
});

process.on('uncaughtException', (error) => {
    logError(`Uncaught exception: ${error.message}`);
    process.exit(1);
});

// Usage information
if (process.argv.includes('--help') || process.argv.includes('-h')) {
    console.log('Usage: node test_websocket.js [OPTIONS]');
    console.log('');
    console.log('Options:');
    console.log('  -i, --interactive    Run in interactive mode');
    console.log('  --url=URL           WebSocket URL (default: ws://localhost:8080/mcp)');
    console.log('  -h, --help          Show this help message');
    console.log('');
    console.log('Examples:');
    console.log('  node test_websocket.js                    # Run automated tests');
    console.log('  node test_websocket.js --interactive      # Run interactive mode');
    console.log('  node test_websocket.js --url=ws://localhost:9000/mcp  # Custom URL');
    process.exit(0);
}

// Run the main function
main().catch((error) => {
    logError(`Fatal error: ${error.message}`);
    process.exit(1);
});
