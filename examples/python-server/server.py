#!/usr/bin/env python3

import asyncio
import json
import logging
import os
import sys
import uuid
from datetime import datetime
from typing import Any, Dict, List, Optional

import click
import websockets
from websockets.server import WebSocketServerProtocol
import numpy as np
import pandas as pd
from sklearn.datasets import make_classification, make_regression
from sklearn.model_selection import train_test_split
from sklearn.linear_model import LinearRegression, LogisticRegression
from sklearn.ensemble import RandomForestClassifier, RandomForestRegressor
from sklearn.metrics import accuracy_score, mean_squared_error, r2_score
import matplotlib
matplotlib.use('Agg')  # Use non-interactive backend
import matplotlib.pyplot as plt
import seaborn as sns
import plotly.graph_objects as go
import plotly.express as px
from io import BytesIO
import base64

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class DataScienceMCPServer:
    def __init__(self, port: int = 5001, host: str = "0.0.0.0"):
        self.port = port
        self.host = host
        self.server_id = os.getenv("MCP_SERVER_ID", f"python-data-science-{port}")
        self.connections: Dict[str, Dict] = {}
        self.datasets: Dict[str, pd.DataFrame] = {}
        self.models: Dict[str, Dict] = {}
        
        # Initialize tools and resources
        self.tools = self._initialize_tools()
        self.resources = self._initialize_resources()
        
    def _initialize_tools(self) -> List[Dict]:
        return [
            {
                "name": "generate_sample_data",
                "description": "Generate sample datasets for testing and analysis",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "dataset_type": {
                            "type": "string",
                            "enum": ["classification", "regression", "timeseries", "random"],
                            "description": "Type of dataset to generate"
                        },
                        "n_samples": {
                            "type": "integer",
                            "description": "Number of samples",
                            "default": 1000
                        },
                        "n_features": {
                            "type": "integer", 
                            "description": "Number of features",
                            "default": 5
                        },
                        "noise": {
                            "type": "number",
                            "description": "Noise level (0.0 to 1.0)",
                            "default": 0.1
                        },
                        "dataset_name": {
                            "type": "string",
                            "description": "Name to store the dataset",
                            "default": "sample_data"
                        }
                    },
                    "required": ["dataset_type"]
                }
            },
            {
                "name": "analyze_data",
                "description": "Perform statistical analysis on a dataset",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "dataset_name": {
                            "type": "string",
                            "description": "Name of the dataset to analyze"
                        },
                        "analysis_type": {
                            "type": "string",
                            "enum": ["descriptive", "correlation", "distribution", "outliers"],
                            "description": "Type of analysis to perform",
                            "default": "descriptive"
                        }
                    },
                    "required": ["dataset_name"]
                }
            },
            {
                "name": "create_visualization",
                "description": "Create data visualizations",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "dataset_name": {
                            "type": "string",
                            "description": "Name of the dataset to visualize"
                        },
                        "chart_type": {
                            "type": "string",
                            "enum": ["histogram", "scatter", "heatmap", "boxplot", "line", "bar"],
                            "description": "Type of chart to create"
                        },
                        "x_column": {
                            "type": "string",
                            "description": "Column for X-axis"
                        },
                        "y_column": {
                            "type": "string",
                            "description": "Column for Y-axis (optional for some charts)"
                        },
                        "title": {
                            "type": "string",
                            "description": "Chart title"
                        },
                        "library": {
                            "type": "string",
                            "enum": ["matplotlib", "seaborn", "plotly"],
                            "description": "Visualization library to use",
                            "default": "matplotlib"
                        }
                    },
                    "required": ["dataset_name", "chart_type"]
                }
            },
            {
                "name": "train_model",
                "description": "Train a machine learning model",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "dataset_name": {
                            "type": "string",
                            "description": "Name of the dataset to use"
                        },
                        "model_type": {
                            "type": "string",
                            "enum": ["linear_regression", "logistic_regression", "random_forest_classifier", "random_forest_regressor"],
                            "description": "Type of model to train"
                        },
                        "target_column": {
                            "type": "string",
                            "description": "Target column name"
                        },
                        "feature_columns": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Feature column names (if not specified, uses all except target)"
                        },
                        "test_size": {
                            "type": "number",
                            "description": "Proportion of dataset for testing",
                            "default": 0.2
                        },
                        "model_name": {
                            "type": "string",
                            "description": "Name to store the trained model",
                            "default": "trained_model"
                        }
                    },
                    "required": ["dataset_name", "model_type", "target_column"]
                }
            },
            {
                "name": "predict",
                "description": "Make predictions using a trained model",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {
                            "type": "string",
                            "description": "Name of the trained model"
                        },
                        "input_data": {
                            "type": "object",
                            "description": "Input data for prediction (column_name: value pairs)"
                        },
                        "dataset_name": {
                            "type": "string",
                            "description": "Name of dataset to predict on (alternative to input_data)"
                        }
                    },
                    "required": ["model_name"]
                }
            },
            {
                "name": "data_transformation",
                "description": "Transform and preprocess data",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "dataset_name": {
                            "type": "string",
                            "description": "Name of the dataset to transform"
                        },
                        "operation": {
                            "type": "string",
                            "enum": ["normalize", "standardize", "encode_categorical", "fill_missing", "remove_outliers"],
                            "description": "Transformation operation"
                        },
                        "columns": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Columns to apply transformation to"
                        },
                        "method": {
                            "type": "string",
                            "description": "Method for transformation (varies by operation)"
                        }
                    },
                    "required": ["dataset_name", "operation"]
                }
            },
            {
                "name": "load_csv_data",
                "description": "Load data from a CSV string or file",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "csv_data": {
                            "type": "string",
                            "description": "CSV data as string"
                        },
                        "dataset_name": {
                            "type": "string",
                            "description": "Name to store the dataset",
                            "default": "loaded_data"
                        },
                        "has_header": {
                            "type": "boolean",
                            "description": "Whether CSV has header row",
                            "default": true
                        }
                    },
                    "required": ["csv_data"]
                }
            },
            {
                "name": "export_data",
                "description": "Export dataset to various formats",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "dataset_name": {
                            "type": "string",
                            "description": "Name of the dataset to export"
                        },
                        "format": {
                            "type": "string",
                            "enum": ["csv", "json", "html"],
                            "description": "Export format",
                            "default": "csv"
                        }
                    },
                    "required": ["dataset_name"]
                }
            }
        ]
    
    def _initialize_resources(self) -> List[Dict]:
        return [
            {
                "uri": "data://datasets",
                "name": "Available Datasets",
                "description": "List of all loaded datasets",
                "mimeType": "application/json"
            },
            {
                "uri": "data://models",
                "name": "Trained Models",
                "description": "List of all trained models",
                "mimeType": "application/json"
            },
            {
                "uri": "data://server-stats",
                "name": "Server Statistics",
                "description": "Server performance and usage statistics",
                "mimeType": "application/json"
            }
        ]

    async def start_server(self):
        """Start the WebSocket server"""
        logger.info(f"[{self.server_id}] Starting Python Data Science MCP Server on {self.host}:{self.port}")
        
        async def handler(websocket: WebSocketServerProtocol, path: str):
            connection_id = str(uuid.uuid4())
            logger.info(f"[{self.server_id}] New connection: {connection_id}")
            
            self.connections[connection_id] = {
                "websocket": websocket,
                "connected_at": datetime.now(),
                "last_activity": datetime.now(),
                "message_count": 0
            }
            
            try:
                async for message in websocket:
                    await self._handle_message(connection_id, message)
            except websockets.exceptions.ConnectionClosed:
                logger.info(f"[{self.server_id}] Connection closed: {connection_id}")
            except Exception as e:
                logger.error(f"[{self.server_id}] Error in connection {connection_id}: {e}")
            finally:
                if connection_id in self.connections:
                    del self.connections[connection_id]
        
        server = await websockets.serve(handler, self.host, self.port)
        logger.info(f"[{self.server_id}] Server listening on {self.host}:{self.port}")
        
        # Start health check server
        await self._start_health_check()
        
        await server.wait_closed()

    async def _start_health_check(self):
        """Start HTTP health check server"""
        from aiohttp import web
        
        async def health_handler(request):
            return web.json_response({
                "status": "healthy",
                "server_id": self.server_id,
                "connections": len(self.connections),
                "datasets": len(self.datasets),
                "models": len(self.models),
                "uptime": (datetime.now() - datetime.now()).total_seconds(),
                "timestamp": datetime.now().isoformat()
            })
        
        app = web.Application()
        app.router.add_get('/health', health_handler)
        
        runner = web.AppRunner(app)
        await runner.setup()
        site = web.TCPSite(runner, self.host, self.port + 1000)
        await site.start()
        
        logger.info(f"[{self.server_id}] Health check endpoint: http://{self.host}:{self.port + 1000}/health")

    async def _handle_message(self, connection_id: str, message: str):
        """Handle incoming WebSocket message"""
        connection = self.connections[connection_id]
        connection["last_activity"] = datetime.now()
        connection["message_count"] += 1
        
        try:
            data = json.loads(message)
            method = data.get("method")
            msg_id = data.get("id")
            params = data.get("params", {})
            
            logger.info(f"[{self.server_id}] Received: {method} ({msg_id}) from {connection_id}")
            
            if method == "initialize":
                await self._handle_initialize(connection["websocket"], msg_id)
            elif method == "ping":
                await self._handle_ping(connection["websocket"], msg_id)
            elif method == "tools/list":
                await self._handle_tools_list(connection["websocket"], msg_id)
            elif method == "tools/call":
                await self._handle_tools_call(connection["websocket"], msg_id, params)
            elif method == "resources/list":
                await self._handle_resources_list(connection["websocket"], msg_id)
            elif method == "resources/read":
                await self._handle_resources_read(connection["websocket"], msg_id, params)
            else:
                await self._send_error(connection["websocket"], msg_id, -32601, f"Method not found: {method}")
                
        except json.JSONDecodeError:
            await self._send_error(connection["websocket"], None, -32700, "Parse error")
        except Exception as e:
            logger.error(f"[{self.server_id}] Error handling message: {e}")
            await self._send_error(connection["websocket"], data.get("id"), -32603, f"Internal error: {str(e)}")

    async def _handle_initialize(self, websocket: WebSocketServerProtocol, msg_id: str):
        """Handle initialize request"""
        response = {
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {"listChanged": False},
                    "resources": {"subscribe": False, "listChanged": False}
                },
                "serverInfo": {
                    "name": self.server_id,
                    "version": "1.0.0",
                    "description": "Python Data Science MCP Server - provides data analysis and ML tools"
                }
            }
        }
        await self._send_message(websocket, response)

    async def _handle_ping(self, websocket: WebSocketServerProtocol, msg_id: str):
        """Handle ping request"""
        response = {
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {}
        }
        await self._send_message(websocket, response)

    async def _handle_tools_list(self, websocket: WebSocketServerProtocol, msg_id: str):
        """Handle tools list request"""
        response = {
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {"tools": self.tools}
        }
        await self._send_message(websocket, response)

    async def _handle_tools_call(self, websocket: WebSocketServerProtocol, msg_id: str, params: Dict):
        """Handle tools call request"""
        tool_name = params.get("name")
        arguments = params.get("arguments", {})
        
        try:
            if tool_name == "generate_sample_data":
                result = await self._generate_sample_data(arguments)
            elif tool_name == "analyze_data":
                result = await self._analyze_data(arguments)
            elif tool_name == "create_visualization":
                result = await self._create_visualization(arguments)
            elif tool_name == "train_model":
                result = await self._train_model(arguments)
            elif tool_name == "predict":
                result = await self._predict(arguments)
            elif tool_name == "data_transformation":
                result = await self._data_transformation(arguments)
            elif tool_name == "load_csv_data":
                result = await self._load_csv_data(arguments)
            elif tool_name == "export_data":
                result = await self._export_data(arguments)
            else:
                raise ValueError(f"Unknown tool: {tool_name}")
            
            response = {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": result
            }
            await self._send_message(websocket, response)
            
        except Exception as e:
            await self._send_error(websocket, msg_id, -32603, f"Tool execution error: {str(e)}")

    async def _generate_sample_data(self, args: Dict) -> Dict:
        """Generate sample datasets"""
        dataset_type = args["dataset_type"]
        n_samples = args.get("n_samples", 1000)
        n_features = args.get("n_features", 5)
        noise = args.get("noise", 0.1)
        dataset_name = args.get("dataset_name", "sample_data")
        
        if dataset_type == "classification":
            X, y = make_classification(
                n_samples=n_samples,
                n_features=n_features,
                n_redundant=0,
                n_informative=n_features,
                n_clusters_per_class=1,
                random_state=42
            )
            # Create DataFrame
            columns = [f"feature_{i}" for i in range(n_features)]
            df = pd.DataFrame(X, columns=columns)
            df["target"] = y
            
        elif dataset_type == "regression":
            X, y = make_regression(
                n_samples=n_samples,
                n_features=n_features,
                noise=noise * 100,
                random_state=42
            )
            columns = [f"feature_{i}" for i in range(n_features)]
            df = pd.DataFrame(X, columns=columns)
            df["target"] = y
            
        elif dataset_type == "timeseries":
            dates = pd.date_range(start="2023-01-01", periods=n_samples, freq="D")
            trend = np.linspace(0, 100, n_samples)
            seasonal = 10 * np.sin(2 * np.pi * np.arange(n_samples) / 365.25)
            noise_vals = np.random.normal(0, noise * 10, n_samples)
            values = trend + seasonal + noise_vals
            
            df = pd.DataFrame({
                "date": dates,
                "value": values,
                "trend": trend,
                "seasonal": seasonal
            })
            
        elif dataset_type == "random":
            data = np.random.randn(n_samples, n_features)
            columns = [f"column_{i}" for i in range(n_features)]
            df = pd.DataFrame(data, columns=columns)
        
        self.datasets[dataset_name] = df
        
        return {
            "content": [
                {
                    "type": "text",
                    "text": f"Generated {dataset_type} dataset '{dataset_name}' with {n_samples} samples and {n_features} features"
                }
            ],
            "metadata": {
                "dataset_name": dataset_name,
                "dataset_type": dataset_type,
                "shape": df.shape,
                "columns": list(df.columns),
                "head": df.head().to_dict()
            }
        }

    async def _analyze_data(self, args: Dict) -> Dict:
        """Perform data analysis"""
        dataset_name = args["dataset_name"]
        analysis_type = args.get("analysis_type", "descriptive")
        
        if dataset_name not in self.datasets:
            raise ValueError(f"Dataset '{dataset_name}' not found")
        
        df = self.datasets[dataset_name]
        
        if analysis_type == "descriptive":
            # Basic descriptive statistics
            numeric_cols = df.select_dtypes(include=[np.number]).columns
            desc_stats = df[numeric_cols].describe()
            
            analysis = {
                "shape": df.shape,
                "columns": list(df.columns),
                "data_types": df.dtypes.to_dict(),
                "missing_values": df.isnull().sum().to_dict(),
                "descriptive_statistics": desc_stats.to_dict()
            }
            
        elif analysis_type == "correlation":
            # Correlation analysis
            numeric_cols = df.select_dtypes(include=[np.number]).columns
            if len(numeric_cols) < 2:
                raise ValueError("Need at least 2 numeric columns for correlation analysis")
            
            corr_matrix = df[numeric_cols].corr()
            analysis = {
                "correlation_matrix": corr_matrix.to_dict(),
                "strong_correlations": []
            }
            
            # Find strong correlations (> 0.7 or < -0.7)
            for i in range(len(corr_matrix.columns)):
                for j in range(i+1, len(corr_matrix.columns)):
                    corr_val = corr_matrix.iloc[i, j]
                    if abs(corr_val) > 0.7:
                        analysis["strong_correlations"].append({
                            "var1": corr_matrix.columns[i],
                            "var2": corr_matrix.columns[j],
                            "correlation": corr_val
                        })
        
        elif analysis_type == "distribution":
            # Distribution analysis
            numeric_cols = df.select_dtypes(include=[np.number]).columns
            analysis = {"distributions": {}}
            
            for col in numeric_cols:
                analysis["distributions"][col] = {
                    "mean": float(df[col].mean()),
                    "median": float(df[col].median()),
                    "std": float(df[col].std()),
                    "skewness": float(df[col].skew()),
                    "kurtosis": float(df[col].kurtosis())
                }
        
        elif analysis_type == "outliers":
            # Outlier detection using IQR method
            numeric_cols = df.select_dtypes(include=[np.number]).columns
            analysis = {"outliers": {}}
            
            for col in numeric_cols:
                Q1 = df[col].quantile(0.25)
                Q3 = df[col].quantile(0.75)
                IQR = Q3 - Q1
                lower_bound = Q1 - 1.5 * IQR
                upper_bound = Q3 + 1.5 * IQR
                
                outliers = df[(df[col] < lower_bound) | (df[col] > upper_bound)]
                analysis["outliers"][col] = {
                    "count": len(outliers),
                    "percentage": len(outliers) / len(df) * 100,
                    "bounds": {"lower": lower_bound, "upper": upper_bound}
                }
        
        return {
            "content": [
                {
                    "type": "text",
                    "text": json.dumps(analysis, indent=2, default=str)
                }
            ],
            "metadata": {
                "dataset_name": dataset_name,
                "analysis_type": analysis_type,
                "analysis": analysis
            }
        }

    async def _create_visualization(self, args: Dict) -> Dict:
        """Create data visualizations"""
        dataset_name = args["dataset_name"]
        chart_type = args["chart_type"]
        x_column = args.get("x_column")
        y_column = args.get("y_column")
        title = args.get("title", f"{chart_type.title()} Chart")
        library = args.get("library", "matplotlib")
        
        if dataset_name not in self.datasets:
            raise ValueError(f"Dataset '{dataset_name}' not found")
        
        df = self.datasets[dataset_name]
        
        # Create visualization based on type and library
        if library == "matplotlib":
            plt.figure(figsize=(10, 6))
            
            if chart_type == "histogram":
                if x_column:
                    plt.hist(df[x_column], bins=30, alpha=0.7)
                    plt.xlabel(x_column)
                else:
                    # Plot histogram for all numeric columns
                    numeric_cols = df.select_dtypes(include=[np.number]).columns
                    for col in numeric_cols[:3]:  # Limit to first 3 columns
                        plt.hist(df[col], bins=30, alpha=0.5, label=col)
                    plt.legend()
                plt.ylabel("Frequency")
                
            elif chart_type == "scatter" and x_column and y_column:
                plt.scatter(df[x_column], df[y_column], alpha=0.6)
                plt.xlabel(x_column)
                plt.ylabel(y_column)
                
            elif chart_type == "line" and x_column and y_column:
                plt.plot(df[x_column], df[y_column])
                plt.xlabel(x_column)
                plt.ylabel(y_column)
                
            elif chart_type == "boxplot":
                if x_column:
                    df.boxplot(column=x_column)
                else:
                    numeric_cols = df.select_dtypes(include=[np.number]).columns
                    df[numeric_cols].boxplot()
            
            plt.title(title)
            plt.tight_layout()
            
            # Convert to base64
            buffer = BytesIO()
            plt.savefig(buffer, format='png', dpi=300, bbox_inches='tight')
            buffer.seek(0)
            image_base64 = base64.b64encode(buffer.getvalue()).decode()
            plt.close()
            
        elif library == "seaborn":
            plt.figure(figsize=(10, 6))
            
            if chart_type == "heatmap":
                numeric_cols = df.select_dtypes(include=[np.number]).columns
                if len(numeric_cols) >= 2:
                    sns.heatmap(df[numeric_cols].corr(), annot=True, cmap='coolwarm', center=0)
                else:
                    raise ValueError("Need at least 2 numeric columns for heatmap")
            elif chart_type == "scatter" and x_column and y_column:
                sns.scatterplot(data=df, x=x_column, y=y_column)
            elif chart_type == "boxplot" and x_column:
                sns.boxplot(data=df, y=x_column)
            
            plt.title(title)
            plt.tight_layout()
            
            buffer = BytesIO()
            plt.savefig(buffer, format='png', dpi=300, bbox_inches='tight')
            buffer.seek(0)
            image_base64 = base64.b64encode(buffer.getvalue()).decode()
            plt.close()
            
        else:
            raise ValueError(f"Library '{library}' not supported yet")
        
        return {
            "content": [
                {
                    "type": "text",
                    "text": f"Created {chart_type} visualization using {library}"
                }
            ],
            "metadata": {
                "chart_type": chart_type,
                "library": library,
                "x_column": x_column,
                "y_column": y_column,
                "title": title
            },
            "visualization": {
                "format": "png",
                "encoding": "base64",
                "data": image_base64
            }
        }

    async def _train_model(self, args: Dict) -> Dict:
        """Train machine learning models"""
        dataset_name = args["dataset_name"]
        model_type = args["model_type"]
        target_column = args["target_column"]
        feature_columns = args.get("feature_columns")
        test_size = args.get("test_size", 0.2)
        model_name = args.get("model_name", "trained_model")
        
        if dataset_name not in self.datasets:
            raise ValueError(f"Dataset '{dataset_name}' not found")
        
        df = self.datasets[dataset_name]
        
        if target_column not in df.columns:
            raise ValueError(f"Target column '{target_column}' not found in dataset")
        
        # Prepare features and target
        if feature_columns:
            X = df[feature_columns]
        else:
            X = df.drop(columns=[target_column])
            # Only use numeric columns
            X = X.select_dtypes(include=[np.number])
        
        y = df[target_column]
        
        # Split data
        X_train, X_test, y_train, y_test = train_test_split(
            X, y, test_size=test_size, random_state=42
        )
        
        # Initialize model
        if model_type == "linear_regression":
            model = LinearRegression()
        elif model_type == "logistic_regression":
            model = LogisticRegression(random_state=42)
        elif model_type == "random_forest_classifier":
            model = RandomForestClassifier(random_state=42)
        elif model_type == "random_forest_regressor":
            model = RandomForestRegressor(random_state=42)
        else:
            raise ValueError(f"Unknown model type: {model_type}")
        
        # Train model
        model.fit(X_train, y_train)
        
        # Make predictions and evaluate
        y_pred = model.predict(X_test)
        
        if model_type in ["linear_regression", "random_forest_regressor"]:
            mse = mean_squared_error(y_test, y_pred)
            r2 = r2_score(y_test, y_pred)
            metrics = {"mse": mse, "r2": r2, "rmse": np.sqrt(mse)}
        else:
            accuracy = accuracy_score(y_test, y_pred)
            metrics = {"accuracy": accuracy}
        
        # Store model
        self.models[model_name] = {
            "model": model,
            "model_type": model_type,
            "feature_columns": list(X.columns),
            "target_column": target_column,
            "metrics": metrics,
            "trained_at": datetime.now().isoformat()
        }
        
        return {
            "content": [
                {
                    "type": "text",
                    "text": f"Trained {model_type} model '{model_name}' successfully"
                }
            ],
            "metadata": {
                "model_name": model_name,
                "model_type": model_type,
                "feature_columns": list(X.columns),
                "target_column": target_column,
                "train_size": len(X_train),
                "test_size": len(X_test),
                "metrics": metrics
            }
        }

    async def _predict(self, args: Dict) -> Dict:
        """Make predictions using trained models"""
        model_name = args["model_name"]
        input_data = args.get("input_data")
        dataset_name = args.get("dataset_name")
        
        if model_name not in self.models:
            raise ValueError(f"Model '{model_name}' not found")
        
        model_info = self.models[model_name]
        model = model_info["model"]
        feature_columns = model_info["feature_columns"]
        
        if input_data:
            # Single prediction from input data
            input_df = pd.DataFrame([input_data])
            if not all(col in input_df.columns for col in feature_columns):
                missing_cols = [col for col in feature_columns if col not in input_df.columns]
                raise ValueError(f"Missing required columns: {missing_cols}")
            
            X = input_df[feature_columns]
            prediction = model.predict(X)[0]
            
            return {
                "content": [
                    {
                        "type": "text",
                        "text": f"Prediction: {prediction}"
                    }
                ],
                "metadata": {
                    "model_name": model_name,
                    "input_data": input_data,
                    "prediction": float(prediction) if isinstance(prediction, (int, float, np.number)) else str(prediction)
                }
            }
        
        elif dataset_name:
            # Batch prediction on dataset
            if dataset_name not in self.datasets:
                raise ValueError(f"Dataset '{dataset_name}' not found")
            
            df = self.datasets[dataset_name]
            X = df[feature_columns]
            predictions = model.predict(X)
            
            # Add predictions to dataset
            df_with_predictions = df.copy()
            df_with_predictions["predictions"] = predictions
            
            return {
                "content": [
                    {
                        "type": "text",
                        "text": f"Made {len(predictions)} predictions on dataset '{dataset_name}'"
                    }
                ],
                "metadata": {
                    "model_name": model_name,
                    "dataset_name": dataset_name,
                    "prediction_count": len(predictions),
                    "predictions_sample": predictions[:10].tolist() if len(predictions) > 10 else predictions.tolist()
                }
            }
        
        else:
            raise ValueError("Either input_data or dataset_name must be provided")

    async def _data_transformation(self, args: Dict) -> Dict:
        """Transform and preprocess data"""
        dataset_name = args["dataset_name"]
        operation = args["operation"]
        columns = args.get("columns")
        method = args.get("method")
        
        if dataset_name not in self.datasets:
            raise ValueError(f"Dataset '{dataset_name}' not found")
        
        df = self.datasets[dataset_name].copy()
        
        if operation == "normalize":
            # Min-max normalization
            if not columns:
                columns = df.select_dtypes(include=[np.number]).columns.tolist()
            
            for col in columns:
                df[col] = (df[col] - df[col].min()) / (df[col].max() - df[col].min())
        
        elif operation == "standardize":
            # Z-score standardization
            if not columns:
                columns = df.select_dtypes(include=[np.number]).columns.tolist()
            
            for col in columns:
                df[col] = (df[col] - df[col].mean()) / df[col].std()
        
        elif operation == "fill_missing":
            method = method or "mean"
            if not columns:
                columns = df.columns[df.isnull().any()].tolist()
            
            for col in columns:
                if method == "mean" and df[col].dtype in [np.number]:
                    df[col] = df[col].fillna(df[col].mean())
                elif method == "median" and df[col].dtype in [np.number]:
                    df[col] = df[col].fillna(df[col].median())
                elif method == "mode":
                    df[col] = df[col].fillna(df[col].mode()[0])
                else:
                    df[col] = df[col].fillna(method)
        
        elif operation == "remove_outliers":
            # Remove outliers using IQR method
            if not columns:
                columns = df.select_dtypes(include=[np.number]).columns.tolist()
            
            for col in columns:
                Q1 = df[col].quantile(0.25)
                Q3 = df[col].quantile(0.75)
                IQR = Q3 - Q1
                lower_bound = Q1 - 1.5 * IQR
                upper_bound = Q3 + 1.5 * IQR
                df = df[(df[col] >= lower_bound) & (df[col] <= upper_bound)]
        
        # Update the dataset
        original_shape = self.datasets[dataset_name].shape
        self.datasets[dataset_name] = df
        
        return {
            "content": [
                {
                    "type": "text",
                    "text": f"Applied {operation} transformation to dataset '{dataset_name}'"
                }
            ],
            "metadata": {
                "dataset_name": dataset_name,
                "operation": operation,
                "columns_affected": columns,
                "original_shape": original_shape,
                "new_shape": df.shape,
                "method": method
            }
        }

    async def _load_csv_data(self, args: Dict) -> Dict:
        """Load data from CSV string"""
        csv_data = args["csv_data"]
        dataset_name = args.get("dataset_name", "loaded_data")
        has_header = args.get("has_header", True)
        
        try:
            from io import StringIO
            
            if has_header:
                df = pd.read_csv(StringIO(csv_data))
            else:
                df = pd.read_csv(StringIO(csv_data), header=None)
            
            self.datasets[dataset_name] = df
            
            return {
                "content": [
                    {
                        "type": "text",
                        "text": f"Loaded CSV data into dataset '{dataset_name}' with shape {df.shape}"
                    }
                ],
                "metadata": {
                    "dataset_name": dataset_name,
                    "shape": df.shape,
                    "columns": list(df.columns),
                    "data_types": df.dtypes.to_dict(),
                    "head": df.head().to_dict()
                }
            }
        except Exception as e:
            raise ValueError(f"Failed to parse CSV data: {str(e)}")

    async def _export_data(self, args: Dict) -> Dict:
        """Export dataset to various formats"""
        dataset_name = args["dataset_name"]
        format_type = args.get("format", "csv")
        
        if dataset_name not in self.datasets:
            raise ValueError(f"Dataset '{dataset_name}' not found")
        
        df = self.datasets[dataset_name]
        
        if format_type == "csv":
            exported_data = df.to_csv(index=False)
        elif format_type == "json":
            exported_data = df.to_json(orient="records", indent=2)
        elif format_type == "html":
            exported_data = df.to_html(index=False)
        else:
            raise ValueError(f"Unsupported format: {format_type}")
        
        return {
            "content": [
                {
                    "type": "text",
                    "text": exported_data
                }
            ],
            "metadata": {
                "dataset_name": dataset_name,
                "format": format_type,
                "shape": df.shape,
                "exported_size": len(exported_data)
            }
        }

    async def _handle_resources_list(self, websocket: WebSocketServerProtocol, msg_id: str):
        """Handle resources list request"""
        response = {
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {"resources": self.resources}
        }
        await self._send_message(websocket, response)

    async def _handle_resources_read(self, websocket: WebSocketServerProtocol, msg_id: str, params: Dict):
        """Handle resources read request"""
        uri = params.get("uri")
        
        if uri == "data://datasets":
            content = {
                "datasets": {
                    name: {
                        "shape": df.shape,
                        "columns": list(df.columns),
                        "data_types": df.dtypes.to_dict()
                    }
                    for name, df in self.datasets.items()
                }
            }
        elif uri == "data://models":
            content = {
                "models": {
                    name: {
                        "model_type": info["model_type"],
                        "feature_columns": info["feature_columns"],
                        "target_column": info["target_column"],
                        "metrics": info["metrics"],
                        "trained_at": info["trained_at"]
                    }
                    for name, info in self.models.items()
                }
            }
        elif uri == "data://server-stats":
            content = {
                "server_id": self.server_id,
                "connections": len(self.connections),
                "datasets": len(self.datasets),
                "models": len(self.models),
                "uptime": "N/A",  # Would need start time tracking
                "memory_usage": "N/A",  # Could add psutil for this
                "timestamp": datetime.now().isoformat()
            }
        else:
            await self._send_error(websocket, msg_id, -32602, f"Unknown resource: {uri}")
            return
        
        response = {
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "contents": [
                    {
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": json.dumps(content, indent=2)
                    }
                ]
            }
        }
        await self._send_message(websocket, response)

    async def _send_message(self, websocket: WebSocketServerProtocol, message: Dict):
        """Send message to WebSocket client"""
        try:
            await websocket.send(json.dumps(message))
        except websockets.exceptions.ConnectionClosed:
            pass

    async def _send_error(self, websocket: WebSocketServerProtocol, msg_id: str, code: int, message: str):
        """Send error message to WebSocket client"""
        error_msg = {
            "jsonrpc": "2.0",
            "id": msg_id,
            "error": {
                "code": code,
                "message": message
            }
        }
        await self._send_message(websocket, error_msg)

@click.command()
@click.option('--port', '-p', default=5001, help='Port to listen on')
@click.option('--host', '-h', default='0.0.0.0', help='Host to bind to')
def main(port: int, host: str):
    """Start the Python Data Science MCP Server"""
    server = DataScienceMCPServer(port=port, host=host)
    
    try:
        # Use uvloop if available for better performance
        try:
            import uvloop
            asyncio.set_event_loop_policy(uvloop.EventLoopPolicy())
        except ImportError:
            pass
        
        asyncio.run(server.start_server())
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
    except Exception as e:
        logger.error(f"Server error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
