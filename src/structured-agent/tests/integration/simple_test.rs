use structured_agent::compiler::CompilationUnit;
use structured_agent::runtime::{ExpressionValue, Runtime};

#[tokio::test]
async fn test_simple_function_call() {
    let program_source = r#"
        fn main(): String {
            "hello world"
        }
    "#;

    let program = CompilationUnit::from_string(program_source.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(ExpressionValue::String(s)) => {
            println!("Success: {}", s);
            assert_eq!(s, "hello world");
        }
        Ok(other) => {
            println!("Unexpected result: {:?}", other);
            panic!("Expected string result");
        }
        Err(e) => {
            println!("Error: {:?}", e);
            panic!("Test failed with error: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_simple_return_statement() {
    let program_source = r#"
        fn main(): String {
            return "returned_value"
        }
    "#;

    let program = CompilationUnit::from_string(program_source.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(ExpressionValue::String(s)) => {
            println!("Success: {}", s);
            assert_eq!(s, "returned_value");
        }
        Ok(other) => {
            println!("Unexpected result: {:?}", other);
            panic!("Expected string result");
        }
        Err(e) => {
            println!("Error: {:?}", e);
            panic!("Test failed with error: {:?}", e);
        }
    }
}
