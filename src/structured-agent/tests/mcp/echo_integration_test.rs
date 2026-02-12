use std::rc::Rc;
use structured_agent::compiler::{CompilationUnit, Compiler};
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

    let program = CompilationUnit::from_string(simple_program.to_string());
    let runtime = Runtime::builder(program)
        .with_compiler(Rc::new(Compiler::new()))
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
    let program_with_extern = r#"
extern fn echo(message: String): String

fn main(): () {
    "Program with extern function parsed"!
}
"#;

    let program = CompilationUnit::from_string(program_with_extern.to_string());
    let runtime = Runtime::builder(program)
        .with_compiler(Rc::new(Compiler::new()))
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
    )
    .await
    .unwrap();

    let test_message = "Hello from MCP integration test!";
    let program_with_extern_call = format!(
        r#"
extern fn echo(message: String): String

fn main(): String {{
    let result = echo("{}")
    result
}}
"#,
        test_message
    );

    let program = CompilationUnit::from_string(program_with_extern_call);
    let runtime = Runtime::builder(program)
        .with_compiler(Rc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "MCP integration test failed: {:?}",
        result.err()
    );

    use structured_agent::runtime::ExpressionValue;
    let value = result.unwrap();
    match value {
        ExpressionValue::String(s) => {
            assert_eq!(
                s, test_message,
                "Expected echo to return '{}', got '{}'",
                test_message, s
            );
            println!("Full MCP integration test passed! Echo returned: {}", s);
        }
        _ => panic!("Expected String result from echo, got {:?}", value),
    }
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
    let greeting = echo("{}")
    greeting
}}
"#,
        test_message
    );

    let program = CompilationUnit::from_string(complete_program);
    let runtime = Runtime::builder(program)
        .with_compiler(Rc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "Complete MCP integration test failed: {:?}",
        result.err()
    );

    use structured_agent::runtime::ExpressionValue;
    let value = result.unwrap();
    match value {
        ExpressionValue::String(s) => {
            assert_eq!(
                s, test_message,
                "Expected echo to return '{}', got '{}'",
                test_message, s
            );
        }
        _ => panic!("Expected String result from echo, got {:?}", value),
    }

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
    let result = echo_with_prefix("{}", "{}")
    result
}}
"#,
        test_message, test_prefix
    );

    let program = CompilationUnit::from_string(program);
    let runtime = Runtime::builder(program)
        .with_compiler(Rc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "echo_with_prefix test failed: {:?}",
        result.err()
    );

    use structured_agent::runtime::ExpressionValue;
    let value = result.unwrap();
    match value {
        ExpressionValue::String(s) => {
            let expected = format!("{}{}", test_prefix, test_message);
            assert_eq!(
                s, expected,
                "Expected echo_with_prefix to return '{}', got '{}'",
                expected, s
            );
            println!("echo_with_prefix test passed! Returned: {}", s);
        }
        _ => panic!(
            "Expected String result from echo_with_prefix, got {:?}",
            value
        ),
    }
}
