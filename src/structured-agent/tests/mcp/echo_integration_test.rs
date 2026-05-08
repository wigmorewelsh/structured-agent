use arrow::array::StringArray;
use std::sync::Arc;
use structured_agent::cli::config::ProgramSource;
use structured_agent::compiler::Compiler;
use structured_agent::mcp::McpClient;
use structured_agent::runtime::Runtime;

#[tokio::test]
async fn test_mcp_client_basic_creation() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await;

    assert!(
        mcp_client.is_ok(),
        "Failed to create MCP client: {:?}",
        mcp_client.err()
    );

    let mcp_client = mcp_client.unwrap();

    let tools = mcp_client.list_tools().await;
    assert!(
        tools.is_ok(),
        "Failed to list tools from MCP server: {:?}",
        tools.err()
    );

    let tools = tools.unwrap();
    assert!(
        !tools.is_empty(),
        "Expected MCP server to provide tools, but got none"
    );

    let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert!(
        tool_names.contains(&"echo".to_string()),
        "Expected 'echo' tool to be available, found: {:?}",
        tool_names
    );

    println!(
        "MCP client created successfully and verified tools: {:?}",
        tool_names
    );
}

#[tokio::test]
async fn test_runtime_with_mcp_client() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let tools = mcp_client.list_tools().await.unwrap();
    assert!(!tools.is_empty(), "MCP server should provide tools");

    let simple_program = r#"
fn main(): () {
    "Hello World"!
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(simple_program.to_string()))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "Simple program execution failed: {:?}",
        result.err()
    );
    println!(
        "Runtime created successfully with MCP client and {} tools available",
        tools.len()
    );
}

#[tokio::test]
async fn test_mcp_echo_external_function_parsing() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let program_with_extern = r#"
extern fn echo(message: String): String

fn main(): () {
    "Program with extern function parsed"!
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program_with_extern.to_string()))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "Program with extern function failed to parse: {:?}",
        result.err()
    );
    println!("Program with extern function parsed successfully");
}

#[tokio::test]
async fn test_mcp_echo_integration_full_pipeline() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let test_message = "Hello from MCP integration test!";
    let program_with_extern_call = format!(
        r#"
extern fn echo(message: String): String

fn main(): String {{
    return echo("{}")
}}
"#,
        test_message
    );

    let runtime = Runtime::builder(ProgramSource::Inline(program_with_extern_call))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "MCP integration test failed: {:?}",
        result.err()
    );

    let value = result.unwrap();
    let s = value.as_string().unwrap();
    assert_eq!(
        s, test_message,
        "Expected echo to return '{}', got '{}'",
        test_message, s
    );
    println!("Full MCP integration test passed! Echo returned: {}", s);
}

#[tokio::test]
async fn test_mcp_complete_integration_workflow() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let tools = mcp_client.list_tools().await.unwrap();
    assert!(!tools.is_empty(), "MCP server should provide tools");

    let has_echo = tools.iter().any(|t| t.name == "echo");
    assert!(has_echo, "MCP server should provide 'echo' tool");

    let test_message = "Hello from structured agent!";
    let complete_program = format!(
        r#"
extern fn echo(message: String): String

fn main(): String {{
    return echo("{}")
}}
"#,
        test_message
    );

    let runtime = Runtime::builder(ProgramSource::Inline(complete_program))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "Complete MCP integration test failed: {:?}",
        result.err()
    );

    let value = result.unwrap();
    let s = value.as_string().unwrap();
    assert_eq!(
        s, test_message,
        "Expected echo to return '{}', got '{}'",
        test_message, s
    );

    println!("Complete MCP integration workflow test passed!");
    println!("  MCP client connection established");
    println!("  External function declaration parsed");
    println!("  MCP tool mapping completed");
    println!("  External function call executed successfully");
    println!("  Echo returned correct value: {}", test_message);
    println!("  End-to-end MCP integration working");
}

#[tokio::test]
async fn test_mcp_echo_with_prefix_tool() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let tools = mcp_client.list_tools().await.unwrap();
    let has_echo_with_prefix = tools.iter().any(|t| t.name == "echo_with_prefix");
    assert!(
        has_echo_with_prefix,
        "MCP server should provide 'echo_with_prefix' tool"
    );

    let test_message = "test message";
    let test_prefix = "PREFIX: ";
    let program = format!(
        r#"
extern fn echo_with_prefix(message: String, prefix: String): String

fn main(): String {{
    return echo_with_prefix("{}", "{}")
}}
"#,
        test_message, test_prefix
    );

    let runtime = Runtime::builder(ProgramSource::Inline(program))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "echo_with_prefix test failed: {:?}",
        result.err()
    );

    let value = result.unwrap();
    let s = value.as_string().unwrap();
    let expected = format!("{}{}", test_prefix, test_message);
    assert_eq!(
        s, expected,
        "Expected echo_with_prefix to return '{}', got '{}'",
        expected, s
    );
}

#[tokio::test]
async fn test_mcp_echo_int_full_pipeline() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let program = r#"
extern fn echo_int(value: Int): Int

fn main(): Int {
    return echo_int(42)
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program.to_string()))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok(), "echo_int test failed: {:?}", result.err());
    assert_eq!(result.unwrap().as_integer().unwrap(), 42);
}

#[tokio::test]
async fn test_mcp_echo_bool_full_pipeline() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let program = r#"
extern fn echo_bool(value: Boolean): Boolean

fn main(): Boolean {
    return echo_bool(true)
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program.to_string()))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok(), "echo_bool test failed: {:?}", result.err());
    assert_eq!(result.unwrap().as_boolean().unwrap(), true);
}

#[tokio::test]
async fn test_mcp_multi_echo_returns_list() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let program = r#"
extern fn multi_echo(messages: List<String>): List<String>

fn main(): List<String> {
    return multi_echo(["hello", "world", "foo"])
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program.to_string()))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok(), "multi_echo test failed: {:?}", result.err());

    let value = result.unwrap();
    let list = value.as_list().unwrap();
    let inner = list.value(0);
    assert_eq!(inner.len(), 3);
    let strings = inner
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(strings.value(0), "hello");
    assert_eq!(strings.value(1), "world");
    assert_eq!(strings.value(2), "foo");
}

#[tokio::test]
async fn test_mcp_single_content_block_still_returns_string() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let program = r#"
extern fn echo(message: String): String

fn main(): String {
    return echo("single block")
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program.to_string()))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "single-block regression test failed: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().as_string().unwrap(), "single block");
}

#[tokio::test]
async fn test_mcp_echo_list_full_pipeline() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "tests/mcp/mcp_echo_server.py".to_string(),
        ],
        None,
    )
    .await
    .unwrap();

    let program = r#"
extern fn echo_list(items: List<String>): List<String>

fn main(): List<String> {
    return echo_list(["a", "b", "c"])
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program.to_string()))
        .with_compiler(Arc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok(), "echo_list test failed: {:?}", result.err());

    let value = result.unwrap();
    let list = value.as_list().unwrap();
    assert_eq!(list.value(0).len(), 3);
}
