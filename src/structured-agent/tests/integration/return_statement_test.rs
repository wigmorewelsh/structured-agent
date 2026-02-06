use structured_agent::compiler::CompilationUnit;
use structured_agent::runtime::{ExprResult, Runtime};

#[tokio::test]
async fn test_return_statement_with_expression() {
    let program_source = r#"
        fn main(): String {
            return "calculated_value"
        }
    "#;

    let program = CompilationUnit::from_string(program_source.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(ExprResult::String(s)) => {
            println!("Success: {}", s);
            assert_eq!(s, "calculated_value");
        }
        Ok(other) => {
            println!("Unexpected result: {:?}", other);
            panic!("Expected string result, got: {:?}", other);
        }
        Err(e) => {
            println!("Error: {:?}", e);
            panic!("Test failed with error: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_return_statement_end_to_end() {
    let program_source = r#"
        fn main(): String {
            let x = "hello"
            return x
            let y = "unreachable"
        }
    "#;

    let program = CompilationUnit::from_string(program_source.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(ExprResult::String(s)) => {
            println!("Success: {}", s);
            assert_eq!(s, "hello");
        }
        Ok(other) => {
            println!("Unexpected result: {:?}", other);
            panic!("Expected string result, got: {:?}", other);
        }
        Err(e) => {
            println!("Error: {:?}", e);
            panic!("Test failed with error: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_return_in_nested_scope() {
    let program_source = r#"
        fn main(): String {
            if true {
                return "from_if_block"
            }
            return "unreachable"
        }
    "#;

    let program = CompilationUnit::from_string(program_source.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(ExprResult::String(s)) => {
            println!("Success: {}", s);
            assert_eq!(s, "from_if_block");
        }
        Ok(other) => {
            println!("Unexpected result: {:?}", other);
            panic!("Expected string result, got: {:?}", other);
        }
        Err(e) => {
            println!("Error: {:?}", e);
            panic!("Test failed with error: {:?}", e);
        }
    }
}
