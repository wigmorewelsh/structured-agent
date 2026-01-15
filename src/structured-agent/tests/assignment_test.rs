use combine::EasyParser;
use structured_agent::compiler::Compiler;
use structured_agent::expressions::Expression;
use structured_agent::parser;
use structured_agent::types::{Context, ExprResult};

#[test]
fn test_assignment_full_pipeline() {
    let code = r#"
fn test_assignment() -> () {
    let message = "Hello, World!"
    message!
}
"#;

    let parse_result = parser::parse_program().easy_parse(code);
    assert!(parse_result.is_ok());

    let (functions, _) = parse_result.unwrap();
    assert_eq!(functions.len(), 1);

    let function = &functions[0];
    assert_eq!(function.name, "test_assignment");
    assert_eq!(function.body.statements.len(), 2);

    let compilation_result = Compiler::compile_function(function);
    assert!(compilation_result.is_ok());
    let compiled_function = compilation_result.unwrap();

    let mut context = Context::new();
    let execution_result = compiled_function.evaluate(&mut context);
    assert!(execution_result.is_ok());

    assert_eq!(context.events.len(), 1);
    assert_eq!(context.events[0].message, "Hello, World!");

    let stored_value = context.get_variable("message");
    assert!(stored_value.is_some());
    match stored_value.unwrap() {
        ExprResult::String(s) => assert_eq!(s, "Hello, World!"),
        _ => panic!("Expected string value in context"),
    }
}

#[test]
fn test_assignment_with_variable_injection() {
    let code = r#"
fn test_var_assignment() -> () {
    let greeting = "Hello"
    let name = "Alice"
    greeting!
    name!
}
"#;

    let (functions, _) = parser::parse_program().easy_parse(code).unwrap();
    let function = &functions[0];
    let compiled_function = Compiler::compile_function(function).unwrap();

    let mut context = Context::new();
    let result = compiled_function.evaluate(&mut context);
    assert!(result.is_ok());

    assert_eq!(context.events.len(), 2);
    assert_eq!(context.events[0].message, "Hello");
    assert_eq!(context.events[1].message, "Alice");

    assert!(context.get_variable("greeting").is_some());
    assert!(context.get_variable("name").is_some());
}

#[test]
fn test_assignment_return_value() {
    let code = r#"
fn test_return() -> () {
    let result = "test value"
}
"#;

    let (functions, _) = parser::parse_program().easy_parse(code).unwrap();
    let function = &functions[0];
    let compiled_function = Compiler::compile_function(function).unwrap();

    let mut context = Context::new();
    let result = compiled_function.evaluate(&mut context);
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
