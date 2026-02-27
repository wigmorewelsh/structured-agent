use structured_agent::compiler::{CompilationUnit, CompilerTrait};
use structured_agent::runtime::Runtime;

#[tokio::test]
async fn test_full_pipeline_parse_compile_execute() {
    // This test now uses the full pipeline with bytecode compiler
    let code = r#"
fn test_func(): () {
    "Hello from function"!
}

fn main(): () {
    test_func()
}
"#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;
    assert!(result.is_ok(), "Program execution failed");

    // Verify the result is Unit type
    let value = result.unwrap();
    assert!(
        matches!(value, structured_agent::runtime::ExpressionValue::Unit),
        "Expected Unit return value, got: {:?}",
        value
    );
}

#[tokio::test]
async fn test_compile_and_execute_with_statements() {
    // Note: BytecodeCompiler compiles whole functions, not individual statements.
    // This test validates that a function with multiple statements executes correctly.
    let code = r#"
fn test(): () {
    "Hello world"!
    let x = "test value"
}

fn main(): () {
    test()
}
"#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;
    assert!(
        result.is_ok(),
        "Program with multiple statements failed to execute"
    );

    let value = result.unwrap();
    assert!(
        matches!(value, structured_agent::runtime::ExpressionValue::Unit),
        "Expected Unit return value"
    );
}

#[tokio::test]
async fn test_variable_injection_after_assignment() {
    let code = r#"
fn test_var_injection(): () {
    let message = "Important message"
    message!
}

fn main(): () {
    test_var_injection()
}
"#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;
    assert!(result.is_ok(), "Variable injection test failed");

    let value = result.unwrap();
    assert!(
        matches!(value, structured_agent::runtime::ExpressionValue::Unit),
        "Expected Unit return value"
    );
}

#[tokio::test]
async fn test_variable_usage() {
    // BytecodeCompiler: Tests variable assignment and usage in a complete function
    let code = r#"
fn test(): String {
    let result = "test value"
    return result
}

fn main(): String {
    return test()
}
"#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;
    assert!(result.is_ok(), "Variable usage test failed");

    let value = result.unwrap();
    match value {
        structured_agent::runtime::ExpressionValue::String(s) => {
            assert_eq!(s, "test value", "Expected 'test value', got: {}", s);
        }
        _ => panic!("Expected String return value, got: {:?}", value),
    }
}

#[tokio::test]
async fn test_compilation_produces_expected_functions() {
    // Verify that bytecode compilation produces the expected function definitions
    let code = r#"
fn helper(x: String): String {
    return "helper"
}

fn main(): String {
    return helper("input")
}
"#;

    let program = CompilationUnit::from_string(code.to_string());
    let compiler = structured_agent::compiler::Compiler::new();
    let compiled = compiler.compile_program(&program);

    assert!(compiled.is_ok(), "Compilation failed");
    let compiled_program = compiled.unwrap();

    // Verify both functions were compiled
    assert_eq!(
        compiled_program.functions().len(),
        2,
        "Expected 2 functions to be compiled"
    );
    assert!(
        compiled_program.functions().contains_key("helper"),
        "Expected 'helper' function to be present"
    );
    assert!(
        compiled_program.functions().contains_key("main"),
        "Expected 'main' function to be present"
    );

    // Verify execution produces correct result
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;
    assert!(result.is_ok(), "Execution failed");

    match result.unwrap() {
        structured_agent::runtime::ExpressionValue::String(s) => {
            assert_eq!(s, "helper", "Expected 'helper' return value");
        }
        other => panic!("Expected String return value, got: {:?}", other),
    }
}
