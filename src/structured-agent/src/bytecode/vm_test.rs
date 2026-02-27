use crate::compiler::CompilationUnit;
use crate::runtime::{ExpressionValue, Runtime};

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
        Ok(ExpressionValue::String(s)) => {
            assert_eq!(s, "hello world");
        }
        Ok(other) => panic!("Expected string result, got: {:?}", other),
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
        Ok(ExpressionValue::String(s)) => {
            assert_eq!(s, "hello");
        }
        Ok(other) => panic!("Expected string result, got: {:?}", other),
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
        Ok(ExpressionValue::String(s)) => {
            assert_eq!(s, "from_if_block");
        }
        Ok(other) => panic!("Expected string result, got: {:?}", other),
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
    match result.unwrap() {
        ExpressionValue::Unit => (),
        other => panic!("Expected Unit result, got: {:?}", other),
    }
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
    match result.unwrap() {
        ExpressionValue::Unit => (),
        other => panic!("Expected Unit result, got: {:?}", other),
    }
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
        Ok(ExpressionValue::String(s)) => {
            assert_eq!(s, "helper_result");
        }
        Ok(other) => panic!("Expected string result, got: {:?}", other),
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
        Ok(ExpressionValue::String(s)) => {
            assert_eq!(s, "input_value");
        }
        Ok(other) => panic!("Expected string result, got: {:?}", other),
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
    match result.unwrap() {
        ExpressionValue::Unit => (),
        other => panic!("Expected Unit result, got: {:?}", other),
    }
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
        Ok(ExpressionValue::Boolean(b)) => {
            assert_eq!(b, true);
        }
        Ok(other) => panic!("Expected boolean result, got: {:?}", other),
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
        Ok(ExpressionValue::String(s)) => {
            assert_eq!(s, "true_branch");
        }
        Ok(other) => panic!("Expected string result, got: {:?}", other),
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
        Ok(ExpressionValue::String(s)) => {
            assert_eq!(s, "false_branch");
        }
        Ok(other) => panic!("Expected string result, got: {:?}", other),
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
        Ok(ExpressionValue::String(s)) => {
            assert_eq!(s, "inner_value");
        }
        Ok(other) => panic!("Expected string result, got: {:?}", other),
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
        Ok(ExpressionValue::Unit) => (),
        Ok(other) => panic!("Expected unit result, got: {:?}", other),
        Err(e) => panic!("Test failed with error: {:?}", e),
    }
}
