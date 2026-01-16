use combine::EasyParser;
use std::rc::Rc;
use structured_agent::compiler::Compiler;
use structured_agent::compiler::parser;
use structured_agent::expressions::Expression;
use structured_agent::runtime::{Context, ExprResult, Runtime};

#[tokio::test]
async fn test_full_pipeline_parse_compile_execute() {
    let code = r#"
fn test_func() -> () {
    "Hello from function"!
}
"#;

    // Parse the code
    let parse_result = parser::parse_program().easy_parse(code);
    assert!(parse_result.is_ok());

    let (functions, _) = parse_result.unwrap();
    assert_eq!(functions.len(), 1);

    let function = &functions[0];
    assert_eq!(function.name, "test_func");
    assert_eq!(function.body.statements.len(), 1);

    // Compile the function
    let compilation_result = Compiler::compile_function(function);
    assert!(compilation_result.is_ok());
    let compiled_function = compilation_result.unwrap();

    // Test that the function can be executed
    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);
    let execution_result = compiled_function.evaluate(&mut context).await;
    assert!(execution_result.is_ok());

    // Check that events were generated (injections)
    assert_eq!(context.events.len(), 1);
    assert_eq!(context.events[0].message, "Hello from function");
}

#[tokio::test]
async fn test_compile_and_execute_individual_statements() {
    let code = r#"
fn test() -> () {
    "Hello world"!
    let x = "test value"
}
"#;

    // Parse
    let (functions, _) = parser::parse_program().easy_parse(code).unwrap();
    let function = &functions[0];

    // Test individual statement compilation and execution
    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);

    // First statement: string injection
    let stmt1 = &function.body.statements[0];
    let compiled_stmt1 = Compiler::compile_statement(stmt1).unwrap();
    let result1 = compiled_stmt1.evaluate(&mut context).await.unwrap();

    match result1 {
        ExprResult::String(s) => assert_eq!(s, "Hello world"),
        _ => panic!("Expected string result"),
    }

    // Check event was added
    assert_eq!(context.events.len(), 1);
    assert_eq!(context.events[0].message, "Hello world");

    // Second statement: assignment (compiles to expression evaluation)
    let stmt2 = &function.body.statements[1];
    let compiled_stmt2 = Compiler::compile_statement(stmt2).unwrap();
    let result2 = compiled_stmt2.evaluate(&mut context).await.unwrap();

    match result2 {
        ExprResult::Unit => {}
        _ => panic!("Expected Unit result from assignment"),
    }

    // Events should still be 1 (assignment doesn't add events)
    assert_eq!(context.events.len(), 1);
}

#[tokio::test]
async fn test_variable_injection_after_assignment() {
    let code = r#"
fn test_var_injection() -> () {
    let message = "Important message"
    message!
}
"#;

    let (functions, _) = parser::parse_program().easy_parse(code).unwrap();
    let function = &functions[0];
    let compiled_function = Compiler::compile_function(function).unwrap();

    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);
    let result = compiled_function.evaluate(&mut context).await;
    assert!(result.is_ok());

    // Should have one event from the variable injection
    assert_eq!(context.events.len(), 1);
    assert_eq!(context.events[0].message, "Important message");
}

#[tokio::test]
async fn test_call_compilation() {
    let code = r#"
fn test() -> () {
let result = "test value"
result!
}
"#;

    let (functions, _) = parser::parse_program().easy_parse(code).unwrap();
    let function = &functions[0];

    // Test call statement compilation
    let stmt1 = &function.body.statements[0];
    let compiled_stmt1 = Compiler::compile_statement(stmt1).unwrap();

    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);
    let result = compiled_stmt1.evaluate(&mut context).await.unwrap();

    match result {
        ExprResult::Unit => {}
        _ => panic!("Expected Unit result from assignment"),
    }

    // Test the second statement (injection)
    let stmt2 = &function.body.statements[1];
    let compiled_stmt2 = Compiler::compile_statement(stmt2).unwrap();
    let result2 = compiled_stmt2.evaluate(&mut context).await.unwrap();

    match result2 {
        ExprResult::String(s) => assert_eq!(s, "test value"),
        _ => panic!("Expected string result from injection"),
    }
}
