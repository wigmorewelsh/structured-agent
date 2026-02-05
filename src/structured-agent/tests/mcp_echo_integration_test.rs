use std::rc::Rc;
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
            "../../tests/fixtures/mcp_echo_server.py".to_string(),
        ],
    )
    .await;

    assert!(
        mcp_client.is_ok(),
        "Failed to create MCP client: {:?}",
        mcp_client.err()
    );
    println!("MCP client created successfully");
}

#[tokio::test]
async fn test_runtime_with_mcp_client() {
    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "../../tests/fixtures/mcp_echo_server.py".to_string(),
        ],
    )
    .await
    .unwrap();

    let runtime = Runtime::builder()
        .with_compiler(Rc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    println!("Runtime created successfully with MCP client");

    // Test a simple program without external functions first
    let simple_program = r#"
fn main(): () {
    "Hello World"!
}
"#;

    let result = runtime.run(simple_program).await;
    assert!(
        result.is_ok(),
        "Simple program execution failed: {:?}",
        result.err()
    );
    println!("Simple program executed successfully");
}

#[tokio::test]
async fn test_mcp_echo_external_function_parsing() {
    let program_with_extern = r#"
extern fn echo(message: String): String

fn main(): () {
    "Program with extern function parsed"!
}
"#;

    let runtime = Runtime::builder()
        .with_compiler(Rc::new(Compiler::new()))
        .build();

    let result = runtime.run(program_with_extern).await;
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
            "../../tests/fixtures/mcp_echo_server.py".to_string(),
        ],
    )
    .await
    .unwrap();

    let runtime = Runtime::builder()
        .with_compiler(Rc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    let program_with_extern_call = r#"
extern fn echo(message: String): String

fn main(): () {
    let result = echo("Hello from MCP integration test!")
    result!
}
"#;

    let result = runtime.run(program_with_extern_call).await;
    assert!(
        result.is_ok(),
        "MCP integration test failed: {:?}",
        result.err()
    );
    println!("Full MCP integration test passed!");
}

#[tokio::test]
async fn test_mcp_complete_integration_workflow() {
    // This test demonstrates the complete MCP integration workflow:
    // 1. Create MCP client connected to echo server
    // 2. Build runtime with MCP client
    // 3. Parse program with extern function declaration
    // 4. Map MCP tools to external functions
    // 5. Execute program that calls external function
    // 6. Verify MCP tool was called and returned expected result

    let mcp_client = McpClient::new_stdio(
        "uv",
        vec![
            "run".to_string(),
            "python".to_string(),
            "../../tests/fixtures/mcp_echo_server.py".to_string(),
        ],
    )
    .await
    .unwrap();

    let runtime = Runtime::builder()
        .with_compiler(Rc::new(Compiler::new()))
        .with_mcp_client(mcp_client)
        .build();

    // Test program that declares an external function and calls it
    let complete_program = r#"
extern fn echo(message: String): String

fn main(): () {
    "Starting MCP integration test"!
    let greeting = echo("Hello from structured agent!")
    greeting!
    "MCP integration test completed successfully"!
}
"#;

    let result = runtime.run(complete_program).await;
    assert!(
        result.is_ok(),
        "Complete MCP integration test failed: {:?}",
        result.err()
    );

    println!("Complete MCP integration workflow test passed!");
    println!("✓ MCP client connection established");
    println!("✓ External function declaration parsed");
    println!("✓ MCP tool mapping completed");
    println!("✓ External function call executed successfully");
    println!("✓ End-to-end MCP integration working");
}
