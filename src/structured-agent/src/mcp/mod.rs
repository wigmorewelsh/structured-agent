use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::{RoleClient, ServiceError, ServiceExt};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

type RmcpClient = rmcp::service::RunningService<RoleClient, ()>;

#[derive(Debug)]
pub enum McpError {
    ConnectionError(String),
    ProtocolError(String),
    ToolError(String),
    SdkError(String),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            McpError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            McpError::ToolError(msg) => write!(f, "Tool error: {}", msg),
            McpError::SdkError(msg) => write!(f, "SDK error: {}", msg),
        }
    }
}

impl Error for McpError {}

impl From<ServiceError> for McpError {
    fn from(e: ServiceError) -> Self {
        McpError::SdkError(e.to_string())
    }
}

impl From<std::io::Error> for McpError {
    fn from(e: std::io::Error) -> Self {
        McpError::ConnectionError(e.to_string())
    }
}

pub struct McpClient {
    client: Arc<RwLock<Option<RmcpClient>>>,
    command: String,
    args: Vec<String>,
}

impl McpClient {
    pub async fn new_stdio(
        command: &str,
        args: Vec<String>,
    ) -> std::result::Result<Self, McpError> {
        Ok(Self {
            client: Arc::new(RwLock::new(None)),
            command: command.to_string(),
            args,
        })
    }

    async fn ensure_connected(&self) -> std::result::Result<(), McpError> {
        let client_lock = self.client.read().await;
        if client_lock.is_none() {
            drop(client_lock);
            self.connect().await?;
        }
        Ok(())
    }

    async fn connect(&self) -> std::result::Result<(), McpError> {
        use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
        use tokio::process::Command;

        let transport = TokioChildProcess::new(Command::new(&self.command).configure(|cmd| {
            for arg in &self.args {
                cmd.arg(arg);
            }
        }))?;

        let service = ()
            .serve(transport)
            .await
            .map_err(|e| McpError::ConnectionError(format!("Failed to start client: {}", e)))?;

        let mut client_lock = self.client.write().await;
        *client_lock = Some(service);

        Ok(())
    }

    pub async fn list_tools(&self) -> std::result::Result<Vec<Tool>, McpError> {
        self.ensure_connected().await?;

        let client_lock = self.client.read().await;
        let client = client_lock
            .as_ref()
            .ok_or_else(|| McpError::ConnectionError("No client available".to_string()))?;

        let tools = client
            .list_all_tools()
            .await
            .map_err(|e| McpError::ProtocolError(format!("Failed to list tools: {}", e)))?;

        Ok(tools)
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> std::result::Result<rmcp::model::CallToolResult, McpError> {
        self.ensure_connected().await?;

        let client_lock = self.client.read().await;
        let client = client_lock
            .as_ref()
            .ok_or_else(|| McpError::ConnectionError("No client available".to_string()))?;

        let params = if let Value::Object(map) = arguments {
            Some(map)
        } else {
            None
        };

        let request = CallToolRequestParams {
            name: name.to_string().into(),
            arguments: params,
            meta: None,
            task: None,
        };

        let response = client
            .call_tool(request)
            .await
            .map_err(|e| McpError::ToolError(format!("Failed to call tool: {}", e)))?;

        Ok(response)
    }

    pub async fn shutdown(&self) -> std::result::Result<(), McpError> {
        let mut client_lock = self.client.write().await;
        if let Some(client) = client_lock.take() {
            client
                .cancel()
                .await
                .map_err(|e| McpError::ConnectionError(format!("Failed to shutdown: {}", e)))?;
        }
        Ok(())
    }
}

pub fn create_client_info(name: &str, version: &str) -> rmcp::model::Implementation {
    rmcp::model::Implementation {
        name: name.into(),
        version: version.into(),
        title: None,
        description: None,
        icons: None,
        website_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_create_client_info() {
        let client_info = create_client_info("test-agent", "0.1.0");
        assert_eq!(client_info.name, "test-agent");
        assert_eq!(client_info.version, "0.1.0");
    }

    #[tokio::test]
    async fn test_mcp_client_creation() {
        let result = McpClient::new_stdio("echo", vec![]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_tools_with_invalid_server() {
        let client = McpClient::new_stdio("echo", vec![]).await.unwrap();
        let result = client.list_tools().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_tool_with_invalid_server() {
        let client = McpClient::new_stdio("echo", vec![]).await.unwrap();
        let result = client.call_tool("test_tool", json!({"arg": "value"})).await;
        assert!(result.is_err());
    }
}
