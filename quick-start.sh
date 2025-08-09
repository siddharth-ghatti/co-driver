#!/bin/bash

# MCP Gateway Quick Start Script
# This script helps you quickly deploy and test the MCP Gateway with example servers

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
COMPOSE_FILE="docker-compose.yml"
HEALTH_CHECK_TIMEOUT=60
TEST_SCRIPT="test-deployment.js"

# Helper functions
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1"
}

header() {
    echo -e "\n${PURPLE}=== $1 ===${NC}\n"
}

check_prerequisites() {
    header "Checking Prerequisites"
    
    # Check Docker
    if command -v docker &> /dev/null; then
        success "Docker is installed"
    else
        error "Docker is not installed. Please install Docker first."
        exit 1
    fi
    
    # Check Docker Compose
    if command -v docker-compose &> /dev/null || docker compose version &> /dev/null; then
        success "Docker Compose is available"
    else
        error "Docker Compose is not available. Please install Docker Compose."
        exit 1
    fi
    
    # Check Node.js (for testing)
    if command -v node &> /dev/null; then
        success "Node.js is installed"
    else
        warning "Node.js is not installed. Deployment testing will be skipped."
    fi
    
    # Check if compose file exists
    if [ -f "$COMPOSE_FILE" ]; then
        success "Docker Compose configuration found"
    else
        error "docker-compose.yml not found. Please run this script from the project root."
        exit 1
    fi
}

deploy_services() {
    header "Deploying MCP Gateway and Servers"
    
    log "Starting services with Docker Compose..."
    
    # Pull latest images if needed
    docker-compose pull --quiet || true
    
    # Start services
    docker-compose up -d
    
    if [ $? -eq 0 ]; then
        success "Services started successfully"
    else
        error "Failed to start services"
        exit 1
    fi
}

wait_for_services() {
    header "Waiting for Services to be Ready"
    
    local services=(
        "http://localhost:8080/health:Gateway"
        "http://localhost:4001/health:File Tools Server"
        "http://localhost:4002/health:Web Tools Server"
    )
    
    local max_attempts=$((HEALTH_CHECK_TIMEOUT / 5))
    
    for service in "${services[@]}"; do
        IFS=':' read -r url name <<< "$service"
        log "Checking $name..."
        
        local attempts=0
        local ready=false
        
        while [ $attempts -lt $max_attempts ] && [ "$ready" = false ]; do
            if curl -s -f "$url" > /dev/null 2>&1; then
                success "$name is ready"
                ready=true
            else
                attempts=$((attempts + 1))
                sleep 5
                log "  Attempt $attempts/$max_attempts for $name..."
            fi
        done
        
        if [ "$ready" = false ]; then
            warning "$name is not responding (this might be OK if the server is optional)"
        fi
    done
}

show_service_status() {
    header "Service Status"
    
    log "Checking running containers..."
    docker-compose ps
    
    echo ""
    log "Service URLs:"
    echo -e "  ${GREEN}Gateway:${NC}           http://localhost:8080"
    echo -e "  ${GREEN}Gateway Health:${NC}    http://localhost:8080/health"
    echo -e "  ${GREEN}Gateway Metrics:${NC}   http://localhost:9090/metrics"
    echo -e "  ${GREEN}Basic MCP Server:${NC}  http://localhost:3001 (& 3002)"
    echo -e "  ${GREEN}File Tools:${NC}        http://localhost:4001"
    echo -e "  ${GREEN}Web Tools:${NC}         http://localhost:4002"
    echo -e "  ${GREEN}Grafana:${NC}           http://localhost:3000 (admin/admin123)"
    echo -e "  ${GREEN}Prometheus:${NC}        http://localhost:9091"
}

run_tests() {
    header "Running Deployment Tests"
    
    if command -v node &> /dev/null && [ -f "$TEST_SCRIPT" ]; then
        log "Running automated deployment tests..."
        
        # Check if ws module is available
        if node -e "require('ws')" 2>/dev/null; then
            node "$TEST_SCRIPT"
        else
            warning "WebSocket module (ws) not installed. Installing..."
            npm install ws 2>/dev/null || {
                warning "Could not install ws module. Skipping automated tests."
                run_manual_tests
            }
            
            if [ $? -eq 0 ]; then
                node "$TEST_SCRIPT"
            else
                run_manual_tests
            fi
        fi
    else
        run_manual_tests
    fi
}

run_manual_tests() {
    log "Running manual tests..."
    
    # Test Gateway Health
    log "Testing gateway health..."
    if curl -s -f http://localhost:8080/health > /dev/null; then
        success "Gateway health check passed"
    else
        error "Gateway health check failed"
    fi
    
    # Test Metrics
    log "Testing metrics endpoint..."
    if curl -s http://localhost:9090/metrics | grep -q "mcp_gateway" 2>/dev/null; then
        success "Metrics endpoint is working"
    else
        warning "Metrics endpoint test inconclusive"
    fi
    
    # Test Basic MCP Server
    log "Testing basic MCP server health..."
    if curl -s -f http://localhost:4001/health > /dev/null; then
        success "Basic MCP server is healthy"
    else
        warning "Basic MCP server health check failed"
    fi
}

show_next_steps() {
    header "Next Steps"
    
    echo -e "Your MCP Gateway deployment is ready! Here's what you can do next:\n"
    
    echo -e "${GREEN}1. Test the WebSocket connection:${NC}"
    echo -e "   wscat -c ws://localhost:8080/mcp"
    echo -e "   # Or use the test script: node test-deployment.js\n"
    
    echo -e "${GREEN}2. View monitoring dashboards:${NC}"
    echo -e "   Open http://localhost:3000 (Grafana - admin/admin123)\n"
    
    echo -e "${GREEN}3. Check metrics:${NC}"
    echo -e "   curl http://localhost:9090/metrics\n"
    
    echo -e "${GREEN}4. View logs:${NC}"
    echo -e "   docker-compose logs -f mcp-gateway\n"
    
    echo -e "${GREEN}5. Scale services:${NC}"
    echo -e "   docker-compose up -d --scale mcp-server-1=3\n"
    
    echo -e "${GREEN}6. Stop services:${NC}"
    echo -e "   docker-compose down\n"
    
    echo -e "${GREEN}7. Read the documentation:${NC}"
    echo -e "   See README.md and DEPLOYMENT.md for detailed information\n"
    
    echo -e "${YELLOW}Configuration files:${NC}"
    echo -e "   Gateway config: config.toml"
    echo -e "   Services config: docker-compose.yml"
    echo -e "   Examples: examples/ directory"
}

cleanup_on_error() {
    if [ $? -ne 0 ]; then
        error "Deployment failed. Cleaning up..."
        docker-compose down 2>/dev/null || true
    fi
}

# Main execution
main() {
    header "MCP Gateway Quick Start"
    
    # Set trap for cleanup
    trap cleanup_on_error EXIT
    
    # Parse command line arguments
    case "${1:-deploy}" in
        "deploy" | "start")
            check_prerequisites
            deploy_services
            wait_for_services
            show_service_status
            run_tests
            show_next_steps
            ;;
        "test")
            run_tests
            ;;
        "status")
            show_service_status
            ;;
        "stop")
            header "Stopping Services"
            docker-compose down
            success "Services stopped"
            ;;
        "clean")
            header "Cleaning Up"
            docker-compose down -v --remove-orphans
            docker system prune -f
            success "Cleanup complete"
            ;;
        "help" | "-h" | "--help")
            echo "MCP Gateway Quick Start Script"
            echo ""
            echo "Usage: $0 [command]"
            echo ""
            echo "Commands:"
            echo "  deploy    Deploy and start all services (default)"
            echo "  start     Alias for deploy"
            echo "  test      Run deployment tests"
            echo "  status    Show service status"
            echo "  stop      Stop all services"
            echo "  clean     Stop services and clean up volumes"
            echo "  help      Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                    # Deploy everything"
            echo "  $0 deploy            # Deploy everything"
            echo "  $0 test              # Run tests only"
            echo "  $0 status            # Check status"
            echo "  $0 stop              # Stop services"
            ;;
        *)
            error "Unknown command: $1"
            echo "Use '$0 help' for usage information."
            exit 1
            ;;
    esac
    
    # Remove trap on successful completion
    trap - EXIT
}

# Run main function
main "$@"
