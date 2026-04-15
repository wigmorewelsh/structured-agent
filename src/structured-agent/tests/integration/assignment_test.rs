use super::helpers::{make_context, parse_and_type_check};
use structured_agent::bytecode::BytecodeCompiler;
use structured_agent::typed_ast;

#[tokio::test]
async fn test_assignment_full_pipeline() {
    let code = r#"
fn test_assignment(): () {
    let message = "Hello, World!"
    message!
}
"#;

    let typed_module = parse_and_type_check(code);

    let functions: Vec<_> = typed_module
        .definitions
        .iter()
        .filter_map(|def| match def {
            typed_ast::Definition::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    let external_functions: Vec<_> = typed_module
        .definitions
        .iter()
        .filter_map(|def| match def {
            typed_ast::Definition::ExternalFunction(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(functions.len(), 1);
    assert_eq!(external_functions.len(), 0);

    let function = &functions[0];
    assert_eq!(function.name, "test_assignment");
    assert_eq!(function.body.statements.len(), 2);

    let compiled_function = BytecodeCompiler::new().compile_function(function).unwrap();
    let (context, _) = compiled_function
        .execute(make_context(), vec![])
        .await
        .unwrap();

    let stored_value = context.get_variable("message");
    assert!(stored_value.is_some());
    assert_eq!(
        stored_value.unwrap().value.as_string().unwrap(),
        "Hello, World!"
    );
}

#[tokio::test]
async fn test_assignment_with_variable_injection() {
    let code = r#"
fn test_var_assignment(): () {
    let greeting = "Hello"
    let name = "Alice"
    greeting!
    name!
}
"#;

    let typed_module = parse_and_type_check(code);

    let functions: Vec<_> = typed_module
        .definitions
        .iter()
        .filter_map(|def| match def {
            typed_ast::Definition::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(functions.len(), 1);

    let compiled_function = BytecodeCompiler::new()
        .compile_function(functions[0])
        .unwrap();
    let (context, _) = compiled_function
        .execute(make_context(), vec![])
        .await
        .unwrap();

    assert_eq!(
        context.events_count(),
        2,
        "Expected 2 events from variable injections"
    );
    assert!(context.get_variable("greeting").is_some());
    assert!(context.get_variable("name").is_some());
}

#[tokio::test]
async fn test_assignment_return_value() {
    let code = r#"
fn test_return(): () {
    let result = "test value"
}
"#;

    let typed_module = parse_and_type_check(code);

    let functions: Vec<_> = typed_module
        .definitions
        .iter()
        .filter_map(|def| match def {
            typed_ast::Definition::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    let external_functions: Vec<_> = typed_module
        .definitions
        .iter()
        .filter_map(|def| match def {
            typed_ast::Definition::ExternalFunction(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(external_functions.len(), 0);

    let compiled_function = BytecodeCompiler::new()
        .compile_function(functions[0])
        .unwrap();
    let (context, expr_result) = compiled_function
        .execute(make_context(), vec![])
        .await
        .unwrap();

    assert_eq!(expr_result.value.type_name(), "Unit");
    assert_eq!(
        context
            .get_variable("result")
            .unwrap()
            .value
            .as_string()
            .unwrap(),
        "test value"
    );
}
