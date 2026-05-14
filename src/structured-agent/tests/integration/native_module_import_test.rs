use super::helpers::run_program_with_unstable;
use arrow::array::{Array, StringArray};

#[tokio::test]
async fn test_head_via_use_without_extern_fn() {
    let source = r#"
use unstable::head

fn main(): Option<String> {
    let list = ["first", "second", "third"]
    return head(list)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert_eq!(result.as_string().unwrap(), "first");
}

#[tokio::test]
async fn test_tail_via_use_without_extern_fn() {
    let source = r#"
use unstable::tail

fn main(): Option<List<String>> {
    let list = ["first", "second", "third"]
    return tail(list)
}
"#;
    let result = run_program_with_unstable(source).await;
    let arr = result.as_list().unwrap();
    assert_eq!(arr.len(), 1);
    let values = arr.value(0);
    let strings = values.as_any().downcast_ref::<StringArray>().unwrap();
    assert_eq!(strings.len(), 2);
    assert_eq!(strings.value(0), "second");
    assert_eq!(strings.value(1), "third");
}

#[tokio::test]
async fn test_is_some_via_use_without_extern_fn() {
    let source = r#"
use unstable::head

fn main(): Option<String> {
    let list = ["x"]
    return head(list)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert_eq!(result.type_name(), "String");
}

#[tokio::test]
async fn test_some_value_via_use_without_extern_fn() {
    let source = r#"
use unstable::head

fn main(): Option<String> {
    let list = ["extracted"]
    return head(list)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert_eq!(result.as_string().unwrap(), "extracted");
}

#[tokio::test]
async fn test_explicit_type_arg_on_generic_call() {
    let source = r#"
use unstable::head

fn first_string(list: List<String>): Option<String> {
    return head<String>(list)
}

fn main(): Option<String> {
    let list = ["hello", "world"]
    return first_string(list)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert_eq!(result.as_string().unwrap(), "hello");
}

#[tokio::test]
async fn test_type_param_passed_through_call_chain() {
    let source = r#"
use unstable::head

fn get_first<T>(list: List<T>): Option<T> {
    return head<T>(list)
}

fn main(): Option<String> {
    let list = ["chained", "result"]
    return get_first<String>(list)
}
"#;
    let result = run_program_with_unstable(source).await;
    assert_eq!(result.as_string().unwrap(), "chained");
}
