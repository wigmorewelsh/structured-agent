use super::helpers::run_program;

#[tokio::test]
async fn test_simple_function_call() {
    let program_source = r#"
        fn main(): String {
            return "hello world"
        }
    "#;

    let value = run_program(program_source).await;
    assert_eq!(value.as_string().unwrap(), "hello world");
}

#[tokio::test]
async fn test_simple_return_statement() {
    let program_source = r#"
        fn main(): String {
            return "returned_value"
        }
    "#;

    let value = run_program(program_source).await;
    assert_eq!(value.as_string().unwrap(), "returned_value");
}
