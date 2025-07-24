#!/bin/bash

# MCP Gateway Test Script
# This script demonstrates the functionality of the MCP Gateway

set -e

echo "🚀 MCP Gateway Test Script"
echo "=========================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
GATEWAY_HOST="localhost"
GATEWAY_PORT="8080"
METRICS_PORT="9090"
GATEWAY_URL="http://${GATEWAY_HOST}:${GATEWAY_PORT}"
METRICS_URL="http://${GATEWAY_HOST}:${METRICS_PORT}"

# Function to print colored output
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Function to check if a service is running
check_service() {
    local url=$1
    local name=$2

    if curl -s --max-time 5 "$url" > /dev/null 2>&1; then
        print_success "$name is running at $url"
        return 0
    else
        print_error "$name is not responding at $url"
        return 1
    fi
}

# Function to test an endpoint
test_endpoint() {
    local url=$1
    local description=$2
    local expected_status=${3:-200}

    print_info "Testing: $description"
    echo "  URL: $url"

    response=$(curl -s -w "HTTPSTATUS:%{http_code}" "$url" 2>/dev/null || echo "HTTPSTATUS:000")
    body=$(echo "$response" | sed -E 's/HTTPSTATUS:[0-9]{3}$//')
    status=$(echo "$response" | grep -o "HTTPSTATUS:[0-9]*" | cut -d: -f2)

    if [ "$status" = "$expected_status" ]; then
        print_success "Status: $status (Expected: $expected_status)"
        if [ -n "$body" ] && [ "$body" != "null" ]; then
            echo "  Response: $(echo "$body" | jq -r . 2>/dev/null || echo "$body")"
        fi
    else
        print_error "Status: $status (Expected: $expected_status)"
        echo "  Response: $body"
    fi
    echo
}

# Function to test WebSocket connection
test_websocket() {
    local ws_url=$1
    local description=$2

    print_info "Testing: $description"
    echo "  WebSocket URL: $ws_url"

    # Create a test MCP message
    local test_message='{"jsonrpc":"2.0","id":1,"method":"ping"}'

    # Use curl to test WebSocket (if available) or skip
    if command -v websocat >/dev/null 2>&1; then
        print_info "Using websocat to test WebSocket connection..."
        echo "$test_message" | timeout 5s websocat "$ws_url" 2>/dev/null || {
            print_warning "WebSocket test failed or timed out (this is expected if no MCP servers are configured)"
        }
    else
        print_warning "websocat not available, skipping WebSocket test"
        print_info "To test WebSocket manually: echo '$test_message' | websocat $ws_url"
    fi
    echo
}

# Start the gateway in background if not already running
start_gateway() {
    if ! check_service "$GATEWAY_URL/health" "Gateway"; then
        print_info "Starting MCP Gateway..."

        # Check if binary exists
        if [ -f "./target/release/mcp-gateway" ]; then
            GATEWAY_BIN="./target/release/mcp-gateway"
        elif [ -f "./target/debug/mcp-gateway" ]; then
            GATEWAY_BIN="./target/debug/mcp-gateway"
        else
            print_error "Gateway binary not found. Please run 'cargo build' first."
            exit 1
        fi

        # Start the gateway
        $GATEWAY_BIN --config config.toml > gateway.log 2>&1 &
        GATEWAY_PID=$!

        print_info "Gateway started with PID: $GATEWAY_PID"
        print_info "Waiting for gateway to start..."

        # Wait for gateway to start (max 30 seconds)
        for i in {1..30}; do
            if check_service "$GATEWAY_URL/health" "Gateway" >/dev/null 2>&1; then
                print_success "Gateway is ready!"
                break
            fi
            if [ $i -eq 30 ]; then
                print_error "Gateway failed to start within 30 seconds"
                print_info "Check gateway.log for details:"
                tail -20 gateway.log
                exit 1
            fi
            sleep 1
        done
    fi
}

# Stop the gateway
stop_gateway() {
    if [ -n "$GATEWAY_PID" ]; then
        print_info "Stopping gateway (PID: $GATEWAY_PID)..."
        kill $GATEWAY_PID 2>/dev/null || true
        wait $GATEWAY_PID 2>/dev/null || true
        print_success "Gateway stopped"
    fi
}

# Cleanup function
cleanup() {
    print_info "Cleaning up..."
    stop_gateway
    exit
}

# Set trap to cleanup on exit
trap cleanup EXIT INT TERM

# Main test execution
main() {
    echo
    print_info "Step 1: Starting the MCP Gateway"
    start_gateway

    echo
    print_info "Step 2: Testing Health Endpoints"
    test_endpoint "$GATEWAY_URL/health" "Health Check"
    test_endpoint "$GATEWAY_URL/readiness" "Readiness Check"
    test_endpoint "$GATEWAY_URL/liveness" "Liveness Check"

    echo
    print_info "Step 3: Testing Status and Management APIs"
    test_endpoint "$GATEWAY_URL/status" "Gateway Status"
    test_endpoint "$GATEWAY_URL/api/v1/connections" "Active Connections"

    echo
    print_info "Step 4: Testing Metrics Endpoint"
    if check_service "$METRICS_URL/metrics" "Metrics Server"; then
        test_endpoint "$METRICS_URL/metrics" "Prometheus Metrics"
    else
        print_warning "Metrics server not enabled or not responding"
    fi

    echo
    print_info "Step 5: Testing WebSocket MCP Endpoint"
    test_websocket "ws://${GATEWAY_HOST}:${GATEWAY_PORT}/mcp" "MCP WebSocket Connection"

    echo
    print_info "Step 6: Testing Error Handling"
    test_endpoint "$GATEWAY_URL/nonexistent" "404 Error Handling" "404"

    echo
    print_success "🎉 All tests completed!"

    echo
    print_info "📊 Gateway Information:"
    echo "  • Main API: $GATEWAY_URL"
    echo "  • WebSocket: ws://${GATEWAY_HOST}:${GATEWAY_PORT}/mcp"
    echo "  • Metrics: $METRICS_URL/metrics"
    echo "  • Health: $GATEWAY_URL/health"
    echo "  • Status: $GATEWAY_URL/status"

    echo
    print_info "📚 Next Steps:"
    echo "  1. Configure upstream MCP servers in config.toml"
    echo "  2. Test with real MCP clients using the WebSocket endpoint"
    echo "  3. Monitor metrics at $METRICS_URL/metrics"
    echo "  4. View logs in gateway.log"

    echo
    print_info "🔗 Useful Commands:"
    echo "  • View logs: tail -f gateway.log"
    echo "  • Test WebSocket: echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}' | websocat ws://localhost:8080/mcp"
    echo "  • Monitor metrics: curl $METRICS_URL/metrics"
    echo "  • Check status: curl $GATEWAY_URL/status | jq"

    echo
    print_warning "Press Ctrl+C to stop the gateway and exit"

    # Keep running until interrupted
    while true; do
        sleep 5
        if ! check_service "$GATEWAY_URL/health" "Gateway" >/dev/null 2>&1; then
            print_error "Gateway appears to have stopped unexpectedly"
            break
        fi
    done
}

# Check dependencies
check_dependencies() {
    local missing_deps=()

    if ! command -v curl >/dev/null 2>&1; then
        missing_deps+=("curl")
    fi

    if ! command -v jq >/dev/null 2>&1; then
        print_warning "jq not found - JSON responses won't be formatted"
    fi

    if ! command -v websocat >/dev/null 2>&1; then
        print_warning "websocat not found - WebSocket tests will be skipped"
        print_info "Install websocat: cargo install websocat"
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        print_error "Missing required dependencies: ${missing_deps[*]}"
        print_info "Please install the missing dependencies and try again"
        exit 1
    fi
}

# Show usage information
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  -h, --help     Show this help message"
    echo "  -q, --quick    Run quick tests only (no WebSocket)"
    echo "  --host HOST    Gateway host (default: localhost)"
    echo "  --port PORT    Gateway port (default: 8080)"
    echo
    echo "Examples:"
    echo "  $0                    # Run all tests"
    echo "  $0 --quick          # Run quick tests only"
    echo "  $0 --host 0.0.0.0   # Test against specific host"
}

# Parse command line arguments
QUICK_MODE=false
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_usage
            exit 0
            ;;
        -q|--quick)
            QUICK_MODE=true
            shift
            ;;
        --host)
            GATEWAY_HOST="$2"
            GATEWAY_URL="http://${GATEWAY_HOST}:${GATEWAY_PORT}"
            shift 2
            ;;
        --port)
            GATEWAY_PORT="$2"
            GATEWAY_URL="http://${GATEWAY_HOST}:${GATEWAY_PORT}"
            shift 2
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Run the tests
check_dependencies
main
