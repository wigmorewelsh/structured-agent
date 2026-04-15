use super::helpers::run_program_with_unstable;

#[tokio::test]
async fn test_head_via_use_without_extern_fn() {
    let source = r#"
use unstable::head
use unstable::some_value

fn main(): String {
    let list = ["first", "second", "third"]
    let h = head(list)
    return some_value(h)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert_eq!(result.as_string().unwrap(), "first");
}

#[tokio::test]
async fn test_tail_via_use_without_extern_fn() {
    let source = r#"
use unstable::tail
use unstable::head
use unstable::some_value

fn main(): String {
    let list = ["first", "second", "third"]
    let t = tail(list)
    let inner = some_value(t)
    let h = head(inner)
    return some_value(h)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert_eq!(result.as_string().unwrap(), "second");
}

#[tokio::test]
async fn test_is_some_via_use_without_extern_fn() {
    let source = r#"
use unstable::head
use unstable::is_some

fn main(): Boolean {
    let list = ["x"]
    let h = head(list)
    return is_some(h)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert!(result.as_boolean().unwrap());
}

#[tokio::test]
async fn test_some_value_via_use_without_extern_fn() {
    let source = r#"
use unstable::head
use unstable::some_value

fn main(): String {
    let list = ["extracted"]
    let h = head(list)
    return some_value(h)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert_eq!(result.as_string().unwrap(), "extracted");
}
