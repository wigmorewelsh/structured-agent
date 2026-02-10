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
async fn test_full_pipeline_parse_compile_execute() {
    let code = r#"
fn test_func(): () {
    "Hello from function"!
}
"#;

    // Parse the code
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
    assert_eq!(function.name, "test_func");
    assert_eq!(function.body.statements.len(), 1);

    // Compile the function
    let compilation_result = Compiler::compile_function(function);
    assert!(compilation_result.is_ok());
    let compiled_function = compilation_result.unwrap();

    // Test that the function can be executed
    let empty_program = CompilationUnit::from_string("fn main() {}".to_string());
    let runtime = Rc::new(Runtime::builder(empty_program).build());
    let context = Arc::new(Context::with_runtime(runtime));
    let execution_result = compiled_function.evaluate(context.clone()).await;
    assert!(execution_result.is_ok());

    // Check that events were generated (injections)
    assert_eq!(context.events_count(), 1);
    assert_eq!(context.get_event(0).unwrap().message, "Hello from function");
}

#[tokio::test]
async fn test_compile_and_execute_individual_statements() {
    let code = r#"
fn test(): () {
    "Hello world"!
    let x = "test value"
}
"#;

    // Parse
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

    // Test individual statement compilation and execution
    let empty_program = CompilationUnit::from_string("fn main() {}".to_string());
    let runtime = Rc::new(Runtime::builder(empty_program).build());
    let context = Arc::new(Context::with_runtime(runtime));

    // First statement: string injection
    let stmt1 = &function.body.statements[0];
    let compiled_stmt1 = Compiler::compile_statement(stmt1).unwrap();
    let result1 = compiled_stmt1.evaluate(context.clone()).await.unwrap();

    match result1.value {
        ExpressionValue::String(s) => assert_eq!(s, "Hello world"),
        _ => panic!("Expected string result"),
    }

    // Check event was added
    assert_eq!(context.events_count(), 1);
    assert_eq!(context.get_event(0).unwrap().message, "Hello world");

    // Second statement: assignment (compiles to expression evaluation)
    let stmt2 = &function.body.statements[1];
    let compiled_stmt2 = Compiler::compile_statement(stmt2).unwrap();
    let result2 = compiled_stmt2.evaluate(context.clone()).await.unwrap();

    match result2.value {
        ExpressionValue::Unit => {}
        _ => panic!("Expected Unit result from assignment"),
    }

    // Events should still be 1 (assignment doesn't add events)
    assert_eq!(context.events_count(), 1);
}

#[tokio::test]
async fn test_variable_injection_after_assignment() {
    let code = r#"
fn test_var_injection(): () {
    let message = "Important message"
    message!
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

    // Should have one event from the variable injection
    assert_eq!(context.events_count(), 1);
    assert_eq!(
        context.get_event(0).unwrap().message,
        "<message>\nImportant message\n</message>"
    );
}

#[tokio::test]
async fn test_call_compilation() {
    let code = r#"
fn test(): () {
let result = "test value"
result!
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

    // Test call statement compilation
    let stmt1 = &function.body.statements[0];
    let compiled_stmt1 = Compiler::compile_statement(stmt1).unwrap();

    let empty_program = CompilationUnit::from_string("fn main() {}".to_string());
    let runtime = Rc::new(Runtime::builder(empty_program).build());
    let context = Arc::new(Context::with_runtime(runtime));
    let result = compiled_stmt1.evaluate(context.clone()).await.unwrap();

    match result.value {
        ExpressionValue::Unit => {}
        _ => panic!("Expected Unit result from assignment"),
    }

    // Test the second statement (injection)
    let stmt2 = &function.body.statements[1];
    let compiled_stmt2 = Compiler::compile_statement(stmt2).unwrap();
    let result2 = compiled_stmt2.evaluate(context.clone()).await.unwrap();

    match result2.value {
        ExpressionValue::String(s) => assert_eq!(s, "test value"),
        _ => panic!("Expected string result from injection"),
    }
}
