use combine::EasyParser;
use std::rc::Rc;
use std::sync::Arc;
use structured_agent::compiler::Compiler;
use structured_agent::compiler::parser;
use structured_agent::expressions::Expression;
use structured_agent::runtime::{Context, ExprResult, Runtime};

#[tokio::test]
async fn test_assignment_full_pipeline() {
    let code = r#"
fn test_assignment() -> () {
    let message = "Hello, World!"
    message!
}
"#;

    let parse_result = parser::parse_program().easy_parse(code);
    assert!(parse_result.is_ok());

    let ((functions, external_functions), _) = parse_result.unwrap();
    assert_eq!(functions.len(), 1);
    assert_eq!(external_functions.len(), 0);

    let function = &functions[0];
    assert_eq!(function.name, "test_assignment");
    assert_eq!(function.body.statements.len(), 2);

    let compilation_result = Compiler::compile_function(function);
    assert!(compilation_result.is_ok());
    let compiled_function = compilation_result.unwrap();

    let runtime = Rc::new(Runtime::new());
    let context = Arc::new(Context::with_runtime(runtime));
    let execution_result = compiled_function.evaluate(context.clone()).await;
    assert!(execution_result.is_ok());

    assert_eq!(context.events_count(), 1);
    assert_eq!(context.get_event(0).unwrap().message, "Hello, World!");

    let stored_value = context.get_variable("message");
    assert!(stored_value.is_some());
    match stored_value.unwrap() {
        ExprResult::String(s) => assert_eq!(s, "Hello, World!"),
        _ => panic!("Expected string value in context"),
    }
}

#[tokio::test]
async fn test_assignment_with_variable_injection() {
    let code = r#"
fn test_var_assignment() -> () {
    let greeting = "Hello"
    let name = "Alice"
    greeting!
    name!
}
"#;

    let ((functions, external_functions), _) = parser::parse_program().easy_parse(code).unwrap();
    assert_eq!(external_functions.len(), 0);
    let function = &functions[0];
    let compiled_function = Compiler::compile_function(function).unwrap();

    let runtime = Rc::new(Runtime::new());
    let context = Arc::new(Context::with_runtime(runtime));
    let result = compiled_function.evaluate(context.clone()).await;
    assert!(result.is_ok());

    assert_eq!(context.events_count(), 2);
    assert_eq!(context.get_event(0).unwrap().message, "Hello");
    assert_eq!(context.get_event(1).unwrap().message, "Alice");

    assert!(context.get_variable("greeting").is_some());
    assert!(context.get_variable("name").is_some());
}

#[tokio::test]
async fn test_assignment_return_value() {
    let code = r#"
fn test_return() -> () {
    let result = "test value"
}
"#;

    let ((functions, external_functions), _) = parser::parse_program().easy_parse(code).unwrap();
    assert_eq!(external_functions.len(), 0);
    let function = &functions[0];
    let compiled_function = Compiler::compile_function(function).unwrap();

    let runtime = Rc::new(Runtime::new());
    let context = Arc::new(Context::with_runtime(runtime));
    let result = compiled_function.evaluate(context.clone()).await;
    assert!(result.is_ok());

    match result.unwrap() {
        ExprResult::Unit => (),
        _ => panic!("Expected unit result"),
    }

    let stored_value = context.get_variable("result").unwrap();
    match stored_value {
        ExprResult::String(s) => assert_eq!(s, "test value"),
        _ => panic!("Expected string in context"),
    }
}
