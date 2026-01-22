use structured_agent::runtime::{ExprResult, Runtime};

#[tokio::test]
async fn test_return_statement_with_expression() {
    let program_source = r#"
        fn main(): String {
            return "calculated_value"
        }
    "#;

    let runtime = Runtime::new();
    let result = runtime.run(program_source).await;

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

    let runtime = Runtime::new();
    let result = runtime.run(program_source).await;

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

    let runtime = Runtime::new();
    let result = runtime.run(program_source).await;

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
