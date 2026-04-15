use super::helpers::run_program;

#[tokio::test]
async fn test_return_statement_with_expression() {
    let program_source = r#"
        fn main(): String {
            return "calculated_value"
        }
    "#;

    let value = run_program(program_source).await;
    assert_eq!(value.as_string().unwrap(), "calculated_value");
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

    let value = run_program(program_source).await;
    assert_eq!(value.as_string().unwrap(), "hello");
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

    let value = run_program(program_source).await;
    assert_eq!(value.as_string().unwrap(), "from_if_block");
}
