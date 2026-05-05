use super::helpers::run_program;
use nonempty::NonEmpty;
use structured_agent::cli::config::ProgramSource;
use structured_agent::compiler::CompilationUnit;
use structured_agent::runtime::Runtime;
use structured_agent_runtime::DefinitionPath;

#[tokio::test]
async fn test_full_pipeline_parse_compile_execute() {
    let code = r#"
fn test_func(): () {
    "Hello from function"!
}

fn main(): () {
    test_func()
}
"#;

    let value = run_program(code).await;
    assert_eq!(value.type_name(), "Unit");
}

#[tokio::test]
async fn test_compile_and_execute_with_statements() {
    let code = r#"
fn test(): () {
    "Hello world"!
    let x = "test value"
}

fn main(): () {
    test()
}
"#;

    let value = run_program(code).await;
    assert_eq!(value.type_name(), "Unit");
}

#[tokio::test]
async fn test_method_call_on_struct() {
    let code = r#"
struct Counter {
    value: Int,
}

impl Counter {
    pub fn get(self): Int {
        return self.value
    }
}

fn main(): Int {
    let c = Counter { value: 42 }
    return c.get()
}
"#;

    let value = run_program(code).await;
    assert_eq!(value.as_integer().unwrap(), 42);
}

#[tokio::test]
async fn test_variable_injection_after_assignment() {
    let code = r#"
fn test_var_injection(): () {
    let message = "Important message"
    message!
}

fn main(): () {
    test_var_injection()
}
"#;

    let value = run_program(code).await;
    assert_eq!(value.type_name(), "Unit");
}

#[tokio::test]
async fn test_variable_usage() {
    let code = r#"
fn test(): String {
    let result = "test value"
    return result
}

fn main(): String {
    return test()
}
"#;

    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "test value");
}

#[tokio::test]
async fn test_compilation_produces_expected_functions() {
    let code = r#"
fn helper(x: String): String {
    return "helper"
}

fn main(): String {
    return helper("input")
}
"#;

    let program = CompilationUnit::from_string(code.to_string());
    let compiled = structured_agent::compiler::Compiler::new().compile_source(&program);

    assert!(compiled.is_ok(), "Compilation failed");
    let compiled_program = compiled.unwrap();

    assert_eq!(
        compiled_program
            .metadata
            .functions
            .values()
            .filter(|d| d.body_ref.is_some())
            .count(),
        2,
        "Expected 2 functions to be compiled"
    );
    assert!(
        compiled_program
            .metadata
            .functions
            .contains_key(&DefinitionPath::for_function(
                DefinitionPath::for_module(NonEmpty::new("main".to_string())),
                "helper",
            )),
        "Expected 'helper' function to be present"
    );
    assert!(
        compiled_program
            .metadata
            .functions
            .contains_key(&DefinitionPath::for_function(
                DefinitionPath::for_module(NonEmpty::new("main".to_string())),
                "main",
            )),
        "Expected 'main' function to be present"
    );

    let value = run_program(code).await;
    assert_eq!(value.as_string().unwrap(), "helper");
}

#[tokio::test]
async fn test_file_based_module_binding_tracer_bullet() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/integration/fixtures/module-binding-tracer/main.sa"
    );
    let value = Runtime::builder(ProgramSource::File(fixture_path.to_string()))
        .build()
        .run()
        .await
        .expect("Program execution failed");
    assert_eq!(value.as_string().unwrap(), "fake module called");
}

#[tokio::test]
async fn test_indirect_call_passes_value_args() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/integration/fixtures/indirect-call-with-args/main.sa"
    );
    let value = Runtime::builder(ProgramSource::File(fixture_path.to_string()))
        .build()
        .run()
        .await
        .expect("Program execution failed");
    assert_eq!(value.as_string().unwrap(), "hello");
}
