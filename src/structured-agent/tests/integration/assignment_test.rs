use combine::Parser;
use combine::stream::position;
use std::rc::Rc;
use std::sync::Arc;
use structured_agent::compiler::parser;
use structured_agent::compiler::{CompilationUnit, Compiler};
use structured_agent::runtime::{Context, ExpressionValue, Runtime};
use structured_agent::types::Expression;
use structured_agent::types::FileId;

const TEST_FILE_ID: FileId = 0;

#[tokio::test]
async fn test_assignment_full_pipeline() {
    let code = r#"
fn test_assignment(): () {
    let message = "Hello, World!"
    message!
}
"#;

    let stream = position::Stream::with_positioner(code, position::IndexPositioner::default());
    let parse_result = parser::parse_program(TEST_FILE_ID).parse(stream);
    assert!(parse_result.is_ok());

    let (module, _) = parse_result.unwrap();
    let functions: Vec<_> = module
        .definitions
        .iter()
        .filter_map(|def| match def {
            structured_agent::ast::Definition::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    let external_functions: Vec<_> = module
        .definitions
        .iter()
        .filter_map(|def| match def {
            structured_agent::ast::Definition::ExternalFunction(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(functions.len(), 1);
    assert_eq!(external_functions.len(), 0);

    let function = &functions[0];
    assert_eq!(function.name, "test_assignment");
    assert_eq!(function.body.statements.len(), 2);

    let compilation_result = Compiler::compile_function(function);
    assert!(compilation_result.is_ok());
    let compiled_function = compilation_result.unwrap();

    let empty_program = CompilationUnit::from_string("fn main() {}".to_string());
    let runtime = Rc::new(Runtime::builder(empty_program).build());
    let context = Arc::new(Context::with_runtime(runtime));
    let execution_result = compiled_function.evaluate(context.clone()).await;
    assert!(execution_result.is_ok());

    assert_eq!(context.events_count(), 1);
    assert_eq!(
        context.get_event(0).unwrap().message,
        "<message>\nHello, World!\n</message>"
    );

    let stored_value = context.get_variable("message");
    assert!(stored_value.is_some());
    match stored_value.unwrap() {
        ExpressionValue::String(s) => assert_eq!(s, "Hello, World!"),
        _ => panic!("Expected string value in context"),
    }
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

    let stream = position::Stream::with_positioner(code, position::IndexPositioner::default());
    let (module, _) = parser::parse_program(TEST_FILE_ID).parse(stream).unwrap();
    let functions: Vec<_> = module
        .definitions
        .iter()
        .filter_map(|def| match def {
            structured_agent::ast::Definition::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(functions.len(), 1);
    let function = functions[0];
    let compiled_function = Compiler::compile_function(function).unwrap();

    let empty_program = CompilationUnit::from_string("fn main() {}".to_string());
    let runtime = Rc::new(Runtime::builder(empty_program).build());
    let context = Arc::new(Context::with_runtime(runtime));
    let result = compiled_function.evaluate(context.clone()).await;
    assert!(result.is_ok());

    assert_eq!(context.events_count(), 2);
    assert_eq!(
        context.get_event(0).unwrap().message,
        "<greeting>\nHello\n</greeting>"
    );
    assert_eq!(
        context.get_event(1).unwrap().message,
        "<name>\nAlice\n</name>"
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

    let stream = position::Stream::with_positioner(code, position::IndexPositioner::default());
    let (module, _) = parser::parse_program(TEST_FILE_ID).parse(stream).unwrap();
    let functions: Vec<_> = module
        .definitions
        .iter()
        .filter_map(|def| match def {
            structured_agent::ast::Definition::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    let external_functions: Vec<_> = module
        .definitions
        .iter()
        .filter_map(|def| match def {
            structured_agent::ast::Definition::ExternalFunction(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(external_functions.len(), 0);
    let function = &functions[0];
    let compiled_function = Compiler::compile_function(function).unwrap();

    let empty_program = CompilationUnit::from_string("fn main() {}".to_string());
    let runtime = Rc::new(Runtime::builder(empty_program).build());
    let context = Arc::new(Context::with_runtime(runtime));
    let result = compiled_function.evaluate(context.clone()).await;
    assert!(result.is_ok());

    match result.unwrap().value {
        ExpressionValue::Unit => (),
        _ => panic!("Expected unit result"),
    }

    let stored_value = context.get_variable("result").unwrap();
    match stored_value {
        ExpressionValue::String(s) => assert_eq!(s, "test value"),
        _ => panic!("Expected string in context"),
    }
}
