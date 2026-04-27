use super::helpers::run_program_with_prelude;

#[tokio::test]
async fn test_native_trait_to_string_int() {
    let source = r#"
fn apply_to_string<T: ToString>(x: T): String {
    return x.to_string()
}
fn main(): String {
    return apply_to_string(42)
}
"#;
    let value = run_program_with_prelude(source).await;
    assert_eq!(value.as_string().unwrap(), "42");
}

#[tokio::test]
async fn test_native_trait_to_string_string() {
    let source = r#"
fn apply_to_string<T: ToString>(x: T): String {
    return x.to_string()
}
fn main(): String {
    return apply_to_string("hello")
}
"#;
    let value = run_program_with_prelude(source).await;
    assert_eq!(value.as_string().unwrap(), "hello");
}
