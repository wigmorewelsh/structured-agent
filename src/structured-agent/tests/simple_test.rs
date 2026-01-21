use structured_agent::runtime::{ExprResult, Runtime};

#[tokio::test]
async fn test_simple_function_call() {
    let program_source = r#"
        fn main() -> String {
            "hello world"
        }
    "#;

    let runtime = Runtime::new();
    let result = runtime.run(program_source).await;

    match result {
        Ok(ExprResult::String(s)) => {
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
        fn main() -> String {
            return "returned_value"
        }
    "#;

    let runtime = Runtime::new();
    let result = runtime.run(program_source).await;

    match result {
        Ok(ExprResult::String(s)) => {
            println!("Return success: {}", s);
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
