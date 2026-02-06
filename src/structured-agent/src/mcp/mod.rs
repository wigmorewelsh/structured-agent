use async_trait::async_trait;

use rust_mcp_sdk::error::McpSdkError;
use rust_mcp_sdk::mcp_client::{McpClientOptions, client_runtime};
use rust_mcp_sdk::schema::*;
use rust_mcp_sdk::{
    McpClient as SdkMcpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
};
use serde_json::Value;
use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

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

impl From<rust_mcp_sdk::TransportError> for McpError {
    fn from(e: rust_mcp_sdk::TransportError) -> Self {
        McpError::ConnectionError(e.to_string())
    }
}

impl From<McpSdkError> for McpError {
    fn from(e: McpSdkError) -> Self {
        McpError::SdkError(e.to_string())
    }
}

pub struct StructuredAgentHandler;

#[async_trait]
impl rust_mcp_sdk::mcp_client::ClientHandler for StructuredAgentHandler {}

pub struct McpClient {
    client: RefCell<Option<Arc<dyn SdkMcpClient>>>,
    command: String,
    args: Vec<String>,
}

impl McpClient {
    pub async fn new_stdio(
        command: &str,
        args: Vec<String>,
    ) -> std::result::Result<Self, McpError> {
        Ok(Self {
            client: RefCell::new(None),
            command: command.to_string(),
            args,
        })
    }

    async fn ensure_connected(&self) -> std::result::Result<(), McpError> {
        if self.client.borrow().is_none() {
            self.connect().await?;
        }
        Ok(())
    }

    async fn connect(&self) -> std::result::Result<(), McpError> {
        let client_details = InitializeRequestParams {
            protocol_version: LATEST_PROTOCOL_VERSION.into(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "structured-agent".into(),
                version: "0.1.0".into(),
                title: Some("Structured Agent MCP Client".into()),
                description: Some("MCP client for structured agent framework".into()),
                icons: vec![],
                website_url: None,
            },
            meta: None,
        };

        let transport = StdioTransport::create_with_server_launch(
            &self.command,
            self.args.clone(),
            None,
            TransportOptions::default(),
        )?;

        let handler = StructuredAgentHandler;

        let client = client_runtime::create_client(McpClientOptions {
            client_details,
            transport,
            handler: handler.to_mcp_client_handler(),
            task_store: None,
            server_task_store: None,
        });

        client.clone().start().await?;

        *self.client.borrow_mut() = Some(client);
        Ok(())
    }

    pub async fn list_tools(&self) -> std::result::Result<Vec<Tool>, McpError> {
        self.ensure_connected().await?;

        let client = self
            .client
            .borrow()
            .as_ref()
            .ok_or_else(|| McpError::ConnectionError("No client available".to_string()))?
            .clone();

        let response = client.request_tool_list(None).await?;
        Ok(response.tools)
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> std::result::Result<CallToolResult, McpError> {
        self.ensure_connected().await?;

        let client = self
            .client
            .borrow()
            .as_ref()
            .ok_or_else(|| McpError::ConnectionError("No client available".to_string()))?
            .clone();

        let params = CallToolRequestParams {
            name: name.to_string(),
            arguments: if let Value::Object(map) = arguments {
                Some(map)
            } else {
                None
            },
            task: None,
            meta: None,
        };
        let response = client.request_tool_call(params).await?;

        Ok(response)
    }

    pub async fn shutdown(&self) -> std::result::Result<(), McpError> {
        let client_option = self.client.borrow().as_ref().cloned();
        if let Some(client) = client_option {
            client.shut_down().await?;
        }
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Some(client) = self.client.borrow_mut().take() {
            std::mem::drop(tokio::spawn(async move {
                let _ = client.shut_down().await;
            }));
        }
    }
}

pub fn create_client_info(name: &str, version: &str) -> Implementation {
    Implementation {
        name: name.to_string(),
        version: version.to_string(),
        title: Some(format!("{} MCP Client", name)),
        description: Some("MCP client for structured agent framework".to_string()),
        icons: vec![],
        website_url: None,
    }
}

pub fn default_capabilities() -> ClientCapabilities {
    ClientCapabilities::default()
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

    #[test]
    fn test_default_capabilities() {
        let _capabilities = default_capabilities();
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
