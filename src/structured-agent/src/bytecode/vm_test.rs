use crate::cli::config::ProgramSource;
use crate::runtime::{ExpressionValue, Runtime};
use arrow::array::{Array, BooleanArray, Int64Array, StringArray, StructArray};

async fn run_program(code: &str) -> ExpressionValue {
    let program = ProgramSource::Inline(code.to_string());
    let runtime = Runtime::builder(program).build();
    runtime.run().await.expect("program failed")
}

#[tokio::test]
async fn test_vm_simple_string_return() {
    let code = r#"
        fn main(): String {
            return "hello world"
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "hello world");
}

#[tokio::test]
async fn test_vm_variable_assignment_and_return() {
    let code = r#"
        fn main(): String {
            let x = "hello"
            return x
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "hello");
}

#[tokio::test]
async fn test_vm_return_in_if_block() {
    let code = r#"
        fn main(): String {
            if true {
                return "from_if_block"
            }
            return "unreachable"
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "from_if_block");
}

#[tokio::test]
async fn test_vm_variable_injection() {
    let code = r#"
        fn main(): () {
            let message = "Hello, World!"
            message!
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.type_name(), "Unit");
}

#[tokio::test]
async fn test_vm_multiple_variable_injections() {
    let code = r#"
        fn main(): () {
            let greeting = "Hello"
            let name = "Alice"
            greeting!
            name!
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.type_name(), "Unit");
}

#[tokio::test]
async fn test_vm_function_call() {
    let code = r#"
        fn helper(): String {
            return "helper_result"
        }

        fn main(): String {
            return helper()
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "helper_result");
}

#[tokio::test]
async fn test_vm_function_call_with_parameter() {
    let code = r#"
        fn helper(x: String): String {
            return x
        }

        fn main(): String {
            return helper("input_value")
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "input_value");
}

#[tokio::test]
async fn test_vm_multiple_statements() {
    let code = r#"
        fn main(): () {
            "Hello world"!
            let x = "test value"
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.type_name(), "Unit");
}

#[tokio::test]
async fn test_vm_boolean_literal() {
    let code = r#"
        fn main(): Boolean {
            return true
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_boolean().unwrap(), true);
}

#[tokio::test]
async fn test_vm_if_else_true_branch() {
    let code = r#"
        fn main(): String {
            if true {
                return "true_branch"
            } else {
                return "false_branch"
            }
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "true_branch");
}

#[tokio::test]
async fn test_vm_if_else_false_branch() {
    let code = r#"
        fn main(): String {
            if false {
                return "true_branch"
            } else {
                return "false_branch"
            }
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "false_branch");
}

#[tokio::test]
async fn test_vm_nested_function_calls() {
    let code = r#"
        fn inner(): String {
            return "inner_value"
        }

        fn outer(): String {
            let result = inner()
            return result
        }

        fn main(): String {
            return outer()
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "inner_value");
}

#[tokio::test]
async fn test_vm_unit_return() {
    let code = r#"
        fn main(): () {
            return ()
        }
    "#;
    let value = run_program(code).await;
    assert_eq!(value.type_name(), "Unit");
}

#[tokio::test]
async fn test_vm_struct_type_recognised_in_llm_generate() {
    let code = r#"
struct Task {
    title: String,
}
fn main(): Task {
    return Task { title: "done" }
}
"#;
    let value = run_program(code).await;
    assert_eq!(value.type_name(), "Struct");
}

#[tokio::test]
async fn test_vm_string_list_literal() {
    let code = r#"
        fn main(): List<String> {
            return ["apple", "banana", "cherry"]
        }
    "#;
    let value = run_program(code).await;
    let list = value.as_list().unwrap();
    assert_eq!(list.len(), 1);
    let values = list.value(0);
    let strings = values.as_any().downcast_ref::<StringArray>().unwrap();
    assert_eq!(strings.len(), 3);
    assert_eq!(strings.value(0), "apple");
    assert_eq!(strings.value(1), "banana");
    assert_eq!(strings.value(2), "cherry");
}

#[tokio::test]
async fn test_vm_int_list_literal() {
    let code = r#"
        fn main(): List<Int> {
            return [1, 2, 3]
        }
    "#;
    let value = run_program(code).await;
    let list = value.as_list().unwrap();
    assert_eq!(list.len(), 1);
    let values = list.value(0);
    let ints = values.as_any().downcast_ref::<Int64Array>().unwrap();
    assert_eq!(ints.len(), 3);
    assert_eq!(ints.value(0), 1);
    assert_eq!(ints.value(1), 2);
    assert_eq!(ints.value(2), 3);
}

#[tokio::test]
async fn test_vm_boolean_list_literal() {
    let code = r#"
        fn main(): List<Boolean> {
            return [true, false, true]
        }
    "#;
    let value = run_program(code).await;
    let list = value.as_list().unwrap();
    assert_eq!(list.len(), 1);
    let values = list.value(0);
    let bools = values.as_any().downcast_ref::<BooleanArray>().unwrap();
    assert_eq!(bools.len(), 3);
    assert_eq!(bools.value(0), true);
    assert_eq!(bools.value(1), false);
    assert_eq!(bools.value(2), true);
}

#[tokio::test]
async fn test_vm_struct_list_literal() {
    let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn main(): List<Point> {
    return [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }]
}
    "#;
    let value = run_program(code).await;
    let list = value.as_list().unwrap();
    assert_eq!(list.len(), 1);
    let values = list.value(0);
    let structs = values.as_any().downcast_ref::<StructArray>().unwrap();
    assert_eq!(structs.len(), 2);
    let x_col = structs
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();
    assert_eq!(x_col.value(0), 1);
    assert_eq!(x_col.value(1), 3);
    let y_col = structs
        .column(1)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();
    assert_eq!(y_col.value(0), 2);
    assert_eq!(y_col.value(1), 4);
}

#[test]
fn test_from_elements_option_some_and_none() {
    use crate::runtime::ExpressionValue;
    use arrow::array::UnionArray;

    let a = ExpressionValue::option_some(ExpressionValue::string("hello"));
    let b = ExpressionValue::option_none_utf8();
    let list = ExpressionValue::from_elements(vec![a, b]).unwrap();
    assert_eq!(list.type_name(), "List");
    let arr = list.as_list().unwrap();
    assert_eq!(arr.len(), 1);
    let values = arr.value(0);
    let unions = values.as_any().downcast_ref::<UnionArray>().unwrap();
    assert_eq!(unions.len(), 2);
    assert_eq!(unions.type_id(0), 1);
    assert_eq!(unions.type_id(1), 0);
}

#[tokio::test]
async fn test_vm_empty_list_literal_is_rejected() {
    let code = r#"
        fn main(): List<String> {
            return []
        }
    "#;
    let program = ProgramSource::Inline(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;
    assert!(
        result.is_err(),
        "Expected type error for empty list literal"
    );
}

#[tokio::test]
async fn test_vm_struct_field_access_in_function() {
    let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn sum_coords(p: Point): Int {
    let xv = p.x
    return xv
}
fn main(): Int {
    let p = Point { x: 5, y: 3 }
    return sum_coords(p)
}
"#;
    let value = run_program(code).await;
    assert_eq!(value.as_integer().unwrap(), 5);
}
