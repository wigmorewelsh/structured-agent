use crate::compiler::CompilationUnit;
use crate::runtime::Runtime;

#[tokio::test]
async fn test_vm_simple_string_return() {
    let code = r#"
        fn main(): String {
            return "hello world"
        }
    "#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_string().unwrap(), "hello world");
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
}

#[tokio::test]
async fn test_vm_variable_assignment_and_return() {
    let code = r#"
        fn main(): String {
            let x = "hello"
            return x
        }
    "#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_string().unwrap(), "hello");
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
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

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_string().unwrap(), "from_if_block");
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
}

#[tokio::test]
async fn test_vm_variable_injection() {
    let code = r#"
        fn main(): () {
            let message = "Hello, World!"
            message!
        }
    "#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    assert!(result.is_ok(), "Expected successful execution");
    let value = result.unwrap();
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

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    assert!(result.is_ok(), "Expected successful execution");
    let value = result.unwrap();
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

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_string().unwrap(), "helper_result");
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
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

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_string().unwrap(), "input_value");
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
}

#[tokio::test]
async fn test_vm_multiple_statements() {
    let code = r#"
        fn main(): () {
            "Hello world"!
            let x = "test value"
        }
    "#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    assert!(result.is_ok(), "Expected successful execution");
    let value = result.unwrap();
    assert_eq!(value.type_name(), "Unit");
}

#[tokio::test]
async fn test_vm_boolean_literal() {
    let code = r#"
        fn main(): Boolean {
            return true
        }
    "#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_boolean().unwrap(), true);
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
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

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_string().unwrap(), "true_branch");
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
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

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_string().unwrap(), "false_branch");
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
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

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) => {
            assert_eq!(value.as_string().unwrap(), "inner_value");
        }
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
}

#[tokio::test]
async fn test_vm_unit_return() {
    let code = r#"
        fn main(): () {
            return ()
        }
    "#;

    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;

    match result {
        Ok(value) if value.type_name() == "Unit" => (),
        Ok(other) => panic!("Expected unit result, got: {:?}", other),
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
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
    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;
    assert!(result.is_ok(), "Expected ok, got: {:?}", result.err());
    let value = result.unwrap();
    assert_eq!(value.type_name(), "Struct");
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
    let program = CompilationUnit::from_string(code.to_string());
    let runtime = Runtime::builder(program).build();
    let result = runtime.run().await;
    assert!(result.is_ok(), "Expected ok, got: {:?}", result.err());
    assert_eq!(result.unwrap().as_integer().unwrap(), 5);
}
