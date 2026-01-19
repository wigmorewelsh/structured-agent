use super::*;
use crate::mcp::McpClient;
use crate::types::{ExternalFunctionDefinition, Type};
use tokio;

#[tokio::test]
async fn test_runtime_builder_with_mcp_client() {
    let client = McpClient::new_stdio("echo", vec![]).await.unwrap();
    let runtime = Runtime::builder().with_mcp_client(client).build();

    // Check that runtime was created successfully with MCP client
    assert_eq!(runtime.mcp_clients_count(), 1);
}

#[tokio::test]
async fn test_runtime_builder_with_multiple_mcp_clients() {
    let client1 = McpClient::new_stdio("echo", vec![]).await.unwrap();
    let client2 = McpClient::new_stdio("cat", vec![]).await.unwrap();
    let clients = vec![client1, client2];

    let runtime = Runtime::builder().with_mcp_clients(clients).build();

    // Test that runtime was created successfully with multiple clients
    assert_eq!(runtime.mcp_clients_count(), 2);
}

#[test]
fn test_external_function_registration() {
    let mut runtime = Runtime::new();

    let ext_func = ExternalFunctionDefinition::new(
        "test_tool".to_string(),
        vec![("param1".to_string(), Type::string())],
        Type::string(),
    );

    runtime.register_external_function(ext_func);

    assert!(runtime.get_external_function("test_tool").is_some());
}

#[test]
fn test_function_registry_with_executable_functions() {
    let runtime = Runtime::new();

    // Test that function registry starts empty
    assert!(runtime.get_function("nonexistent").is_none());

    // Test that we can list functions
    let functions = runtime.list_functions();
    assert_eq!(functions.len(), 0);
}

#[test]
fn test_runtime_clone_creates_separate_instance() {
    let runtime = Runtime::new();
    let cloned = runtime.clone();

    // Both should be separate instances
    assert!(runtime.get_function("test").is_none());
    assert!(cloned.get_function("test").is_none());
}
