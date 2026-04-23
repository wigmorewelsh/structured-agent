use super::helpers::{make_context, parse_and_type_check};
use nonempty::NonEmpty;
use structured_agent::bytecode::{BytecodeCompiler, BytecodeFunctionExpr, BytecodeRef};
use structured_agent::typed_ast;
use structured_agent::types::Function;
use structured_agent_runtime::DefinitionPath;

fn make_bytecode_ref(compiled: structured_agent::bytecode::CompiledFunction) -> BytecodeRef {
    BytecodeRef {
        instructions: compiled.instructions,
        labels: compiled.labels,
        parameters: compiled.parameters,
        return_type: compiled.return_type,
        documentation: compiled.documentation,
        slot_table: compiled.slot_table,
    }
}

fn make_path(name: &str) -> DefinitionPath {
    DefinitionPath::for_function(
        DefinitionPath::for_module(NonEmpty::new("test".to_string())),
        name,
    )
}

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
    assert_eq!(functions.len(), 1);

    let function = &functions[0];
    assert_eq!(function.name, "test_assignment");

    let compiled = BytecodeCompiler::new()
        .compile_to_bytecode(function)
        .unwrap();
    let body_ref = make_bytecode_ref(compiled);
    let func_expr = BytecodeFunctionExpr::new(make_path("test_assignment"), body_ref);
    let (context, _) = func_expr.execute(make_context(), vec![]).await.unwrap();

    assert_eq!(context.events_count(), 1);
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

    let compiled = BytecodeCompiler::new()
        .compile_to_bytecode(functions[0])
        .unwrap();
    let body_ref = make_bytecode_ref(compiled);
    let func_expr = BytecodeFunctionExpr::new(make_path("test_var_assignment"), body_ref);
    let (context, _) = func_expr.execute(make_context(), vec![]).await.unwrap();

    assert_eq!(context.events_count(), 2);
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
    assert_eq!(functions.len(), 1);

    let compiled = BytecodeCompiler::new()
        .compile_to_bytecode(functions[0])
        .unwrap();
    let body_ref = make_bytecode_ref(compiled);
    let func_expr = BytecodeFunctionExpr::new(make_path("test_return"), body_ref);
    let (_context, expr_result) = func_expr.execute(make_context(), vec![]).await.unwrap();

    assert_eq!(expr_result.value.type_name(), "Unit");
}
