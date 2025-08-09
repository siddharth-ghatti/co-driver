# MCP Server Deployment Guide

This document provides comprehensive examples and instructions for deploying MCP (Model Context Protocol) servers that work with the MCP Gateway.

## Overview

The MCP Gateway supports multiple upstream MCP servers with load balancing, health checks, and routing. This guide shows you how to deploy various types of MCP servers in different environments.

## Quick Start with Docker Compose

The fastest way to get started is using the provided Docker Compose configuration:

```bash
# Clone the repository
git clone <repository-url>
cd mcp-gateway

# Start all services (gateway + example servers)
docker-compose up -d

# Check status
docker-compose ps

# View logs
docker-compose logs -f mcp-gateway
```

This will start:
- **MCP Gateway** on port 8080
- **Basic MCP Server 1** on port 3001  
- **Basic MCP Server 2** on port 3002
- **File Tools Server** on port 4001
- **Web Tools Server** on port 4002
- **Redis** for caching on port 6379
- **Prometheus** for metrics on port 9091
- **Grafana** for dashboards on port 3000

## Individual Server Deployment

### 1. Basic Node.js MCP Server

The basic server provides fundamental MCP protocol features.

#### Manual Deployment

```bash
cd examples/mcp-server

# Install dependencies
npm install

# Start server
npm start

# Or with custom configuration
node server.js --port 3001 --host 0.0.0.0
```

#### Docker Deployment

```bash
# Build image
docker build -t mcp-basic-server examples/mcp-server

# Run container
docker run -d \
  --name mcp-server-1 \
  -p 3001:3001 \
  -e MCP_SERVER_ID=mcp-server-1 \
  mcp-basic-server
```

#### Environment Variables

- `MCP_SERVER_ID`: Unique server identifier
- `NODE_ENV`: Environment (development/production)

### 2. File Tools Server

Provides file system operations and management.

#### Manual Deployment

```bash
cd examples/file-tools-server

# Install dependencies
npm install

# Create shared directory
mkdir -p /tmp/mcp-files

# Start server
SHARED_PATH=/tmp/mcp-files node server.js --port 4001
```

#### Docker Deployment

```bash
# Build image
docker build -t mcp-file-tools examples/file-tools-server

# Run with volume mount
docker run -d \
  --name file-tools \
  -p 4001:4001 \
  -v ./shared-files:/shared \
  -e SHARED_PATH=/shared \
  -e MCP_SERVER_ID=file-tools \
  mcp-file-tools
```

### 3. Web Tools Server

Provides web scraping and HTTP utilities.

#### Manual Deployment

```bash
cd examples/web-tools-server

# Install dependencies
npm install

# Start server (basic mode)
node server.js --port 4002

# Start with browser automation (requires Puppeteer)
node server.js --port 4002 --browser
```

#### Docker Deployment

```bash
# Build image with Puppeteer support
docker build -t mcp-web-tools examples/web-tools-server

# Run container
docker run -d \
  --name web-tools \
  -p 4002:4002 \
  --shm-size=1gb \
  -e MCP_SERVER_ID=web-tools \
  mcp-web-tools --browser
```

### 4. Python Data Science Server

Provides data analysis and machine learning capabilities.

#### Manual Deployment

```bash
cd examples/python-server

# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt

# Start server
python server.py --port 5001
```

#### Docker Deployment

```dockerfile
# Dockerfile for Python server
FROM python:3.11-slim

WORKDIR /app
COPY requirements.txt .
RUN pip install -r requirements.txt

COPY server.py .
EXPOSE 5001

CMD ["python", "server.py", "--port", "5001", "--host", "0.0.0.0"]
```

```bash
# Build and run
docker build -t mcp-python-server examples/python-server
docker run -d --name python-server -p 5001:5001 mcp-python-server
```

## Production Deployment

### Kubernetes Deployment

#### Gateway Deployment

```yaml
# mcp-gateway-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mcp-gateway
spec:
  replicas: 3
  selector:
    matchLabels:
      app: mcp-gateway
  template:
    metadata:
      labels:
        app: mcp-gateway
    spec:
      containers:
      - name: mcp-gateway
        image: mcp-gateway:latest
        ports:
        - containerPort: 8080
        - containerPort: 9090
        env:
        - name: RUST_LOG
          value: "info"
        volumeMounts:
        - name: config
          mountPath: /app/config.toml
          subPath: config.toml
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /readiness
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: mcp-gateway-config
---
apiVersion: v1
kind: Service
metadata:
  name: mcp-gateway-service
spec:
  selector:
    app: mcp-gateway
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090
  type: LoadBalancer
```

#### MCP Server Deployment

```yaml
# mcp-servers-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mcp-basic-servers
spec:
  replicas: 2
  selector:
    matchLabels:
      app: mcp-basic-server
  template:
    metadata:
      labels:
        app: mcp-basic-server
    spec:
      containers:
      - name: mcp-server
        image: node:18-alpine
        workingDir: /app
        command: ["sh", "-c"]
        args:
        - |
          npm install &&
          node server.js --port 3001 --host 0.0.0.0
        ports:
        - containerPort: 3001
        env:
        - name: MCP_SERVER_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        volumeMounts:
        - name: server-code
          mountPath: /app
        livenessProbe:
          httpGet:
            path: /health
            port: 4001
          initialDelaySeconds: 30
          periodSeconds: 10
      volumes:
      - name: server-code
        configMap:
          name: mcp-server-code
---
apiVersion: v1
kind: Service
metadata:
  name: mcp-basic-server-service
spec:
  selector:
    app: mcp-basic-server
  ports:
  - port: 3001
    targetPort: 3001
  clusterIP: None  # Headless service for individual server discovery
```

### Helm Chart Deployment

```yaml
# values.yaml
gateway:
  replicaCount: 3
  image:
    repository: mcp-gateway
    tag: latest
  service:
    type: LoadBalancer
    port: 80

servers:
  basic:
    enabled: true
    replicaCount: 2
    port: 3001
  
  fileTools:
    enabled: true
    replicaCount: 1
    port: 4001
    storage:
      size: 10Gi
  
  webTools:
    enabled: true
    replicaCount: 1
    port: 4002
    browser: true

monitoring:
  prometheus:
    enabled: true
  grafana:
    enabled: true

redis:
  enabled: true
```

```bash
# Deploy with Helm
helm install mcp-stack ./charts/mcp-gateway -f values.yaml
```

### AWS ECS Deployment

#### Task Definition

```json
{
  "family": "mcp-gateway-stack",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "1024",
  "memory": "2048",
  "executionRoleArn": "arn:aws:iam::account:role/ecsTaskExecutionRole",
  "containerDefinitions": [
    {
      "name": "mcp-gateway",
      "image": "your-account.dkr.ecr.region.amazonaws.com/mcp-gateway:latest",
      "portMappings": [
        {"containerPort": 8080, "protocol": "tcp"},
        {"containerPort": 9090, "protocol": "tcp"}
      ],
      "environment": [
        {"name": "RUST_LOG", "value": "info"}
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/mcp-gateway",
          "awslogs-region": "us-west-2",
          "awslogs-stream-prefix": "ecs"
        }
      }
    },
    {
      "name": "mcp-server",
      "image": "your-account.dkr.ecr.region.amazonaws.com/mcp-basic-server:latest",
      "portMappings": [
        {"containerPort": 3001, "protocol": "tcp"}
      ],
      "environment": [
        {"name": "MCP_SERVER_ID", "value": "mcp-server-ecs"}
      ]
    }
  ]
}
```

#### Service Definition

```json
{
  "serviceName": "mcp-gateway-service",
  "cluster": "mcp-cluster",
  "taskDefinition": "mcp-gateway-stack",
  "desiredCount": 2,
  "launchType": "FARGATE",
  "networkConfiguration": {
    "awsvpcConfiguration": {
      "subnets": ["subnet-12345", "subnet-67890"],
      "securityGroups": ["sg-12345"],
      "assignPublicIp": "ENABLED"
    }
  },
  "loadBalancers": [
    {
      "targetGroupArn": "arn:aws:elasticloadbalancing:region:account:targetgroup/mcp-gateway/123456789",
      "containerName": "mcp-gateway",
      "containerPort": 8080
    }
  ]
}
```

### Google Cloud Run Deployment

```yaml
# cloud-run-gateway.yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: mcp-gateway
  annotations:
    run.googleapis.com/ingress: all
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/maxScale: "10"
        run.googleapis.com/cpu-throttling: "false"
    spec:
      containerConcurrency: 100
      containers:
      - name: mcp-gateway
        image: gcr.io/project-id/mcp-gateway:latest
        ports:
        - containerPort: 8080
        env:
        - name: RUST_LOG
          value: info
        resources:
          limits:
            cpu: "1"
            memory: "1Gi"
```

```bash
# Deploy to Cloud Run
gcloud run deploy mcp-gateway \
  --image gcr.io/project-id/mcp-gateway:latest \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --port 8080 \
  --cpu 1 \
  --memory 1Gi \
  --max-instances 10
```

## Scaling and Load Balancing

### Horizontal Scaling

#### Docker Compose Scaling

```bash
# Scale basic servers to 5 instances
docker-compose up -d --scale mcp-server-1=3 --scale mcp-server-2=2

# Scale file tools servers
docker-compose up -d --scale file-tools=2
```

#### Kubernetes Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: mcp-servers-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: mcp-basic-servers
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

### Load Balancing Strategies

Update the gateway configuration for different load balancing:

```toml
# Round Robin (default)
[upstream.groups.default]
strategy = "round_robin"

# Weighted Round Robin
[upstream.groups.weighted]
strategy = "weighted_round_robin"

[[upstream.groups.weighted.servers]]
id = "high-performance-server"
url = "ws://high-perf:3001"
weight = 200

[[upstream.groups.weighted.servers]]
id = "standard-server"
url = "ws://standard:3001"
weight = 100

# Least Connections
[upstream.groups.least_conn]
strategy = "least_connections"

# IP Hash (session affinity)
[upstream.groups.sticky]
strategy = "ip_hash"
```

## Monitoring and Observability

### Prometheus Metrics

All servers expose metrics on `/metrics` endpoint:

```bash
# Gateway metrics
curl http://gateway:9090/metrics

# Server metrics (if implemented)
curl http://mcp-server:4001/metrics
```

### Grafana Dashboards

Import the provided dashboard:

```bash
# Copy dashboard configuration
cp monitoring/grafana/dashboards/mcp-gateway.json /var/lib/grafana/dashboards/
```

### Logging

#### Centralized Logging with ELK Stack

```yaml
# filebeat.yml
filebeat.inputs:
- type: docker
  containers.ids:
  - "*"
  processors:
  - add_docker_metadata: ~

output.elasticsearch:
  hosts: ["elasticsearch:9200"]

logging.level: info
```

#### Structured Logging

Configure servers for structured logging:

```bash
# Node.js servers
export LOG_FORMAT=json
export LOG_LEVEL=info

# Python server
export PYTHONPATH=/app
export LOG_LEVEL=INFO

# Rust gateway
export RUST_LOG=info
export LOG_FORMAT=json
```

## Security Considerations

### TLS/SSL Configuration

```toml
# Gateway TLS configuration
[security.tls]
cert_file = "/etc/ssl/certs/gateway.crt"
key_file = "/etc/ssl/private/gateway.key"
ca_file = "/etc/ssl/certs/ca.crt"
verify_client = false
```

### Authentication

```toml
# JWT Authentication
[security.auth]
enabled = true
jwt_secret = "your-super-secret-jwt-signing-key"
jwt_issuer = "mcp-gateway"
jwt_audience = "mcp-clients"
jwt_expiration = 3600

# API Key Authentication
api_key_header = "X-API-Key"

# Basic Authentication
[security.auth.basic_auth_users]
admin = "hashed-password"
```

### Network Security

```bash
# Docker network isolation
docker network create mcp-internal

# Run servers on internal network
docker run -d --network mcp-internal mcp-server
```

### Kubernetes Network Policies

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: mcp-network-policy
spec:
  podSelector:
    matchLabels:
      app: mcp-server
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          app: mcp-gateway
    ports:
    - protocol: TCP
      port: 3001
```

## Troubleshooting

### Common Issues

#### Connection Refused

```bash
# Check if server is running
curl http://localhost:3001/health

# Check logs
docker logs mcp-server-1

# Check network connectivity
telnet localhost 3001
```

#### High Memory Usage

```bash
# Monitor memory usage
docker stats

# Check for memory leaks in Node.js
node --inspect server.js
```

#### Performance Issues

```bash
# Monitor metrics
curl http://localhost:9090/metrics | grep mcp_gateway

# Check connection pool
curl http://localhost:8080/api/v1/upstreams
```

### Debugging Tools

```bash
# WebSocket testing
wscat -c ws://localhost:8080/mcp

# HTTP testing
curl -X POST http://localhost:8080/api/v1/test \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "method": "ping", "id": 1}'

# Load testing
artillery run load-test.yml
```

## Best Practices

### 1. Health Checks
- Implement comprehensive health checks
- Use different endpoints for liveness and readiness
- Include dependency checks (database, external services)

### 2. Resource Management
- Set appropriate CPU and memory limits
- Use connection pooling
- Implement graceful shutdown

### 3. Monitoring
- Monitor key metrics (latency, error rate, throughput)
- Set up alerting for critical issues
- Use distributed tracing for complex requests

### 4. Security
- Use TLS for all communications
- Implement proper authentication and authorization
- Regular security updates and vulnerability scanning

### 5. Scalability
- Design stateless servers
- Use horizontal scaling over vertical
- Implement circuit breakers and rate limiting

This deployment guide provides comprehensive examples for running MCP servers in various environments, from local development to production cloud deployments.
