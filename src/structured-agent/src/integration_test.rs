#[cfg(test)]
mod tests {
    use crate::cli::config::ProgramSource;
    use crate::compiler::{CompilationUnit, CompiledProgram, Compiler};
    use crate::runtime::Runtime;
    use crate::typecheck::TypeChecker;

    fn compile(code: &str) -> Result<CompiledProgram, String> {
        let unit = CompilationUnit::from_string(code.to_string());
        Compiler::new().compile_source(&unit)
    }

    #[test]
    fn test_type_checker_integration_valid_program() {
        let code = r#"
fn greet(name: String): String {
    return name
}

fn main(): () {
    let greeting = greet("Alice")
}
"#;

        let result = compile(code);

        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(result.is_ok(), "Valid program should compile successfully");
    }

    #[test]
    fn test_type_checker_integration_type_error() {
        let code = r#"
fn greet(name: String): String {
    return name
}

fn main(): () {
    let greeting = greet(true)
}
"#;

        let result = compile(code);

        if result.is_ok() {
            println!("Expected error but compilation succeeded");
        }
        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(
            result.is_err(),
            "Program with type error should fail to compile"
        );
        let err = result.unwrap_err();
        assert!(err.contains("mismatched argument type"));
        assert!(err.contains("in function `greet`, parameter `name`"));
    }

    #[test]
    fn test_type_checker_integration_return_type_mismatch() {
        let code = r#"
fn get_number(): String {
    return true
}
"#;

        let result = compile(code);

        if result.is_ok() {
            println!("Expected error but compilation succeeded");
        }
        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(
            result.is_err(),
            "Return type mismatch should fail to compile"
        );
        let err = result.unwrap_err();
        assert!(err.contains("mismatched return type"));
        assert!(err.contains("in function `get_number`"));
    }

    #[test]
    fn test_type_checker_integration_placeholder_arguments() {
        let code = r#"
fn process(data: String): () {
}

fn main(): () {
    process(_)
}
"#;

        let result = compile(code);

        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(result.is_ok(), "Placeholder arguments should be allowed");
    }

    #[test]
    fn test_type_checker_integration_select_statement() {
        let code = r#"
fn get_string(): String {
    return "hello"
}

fn get_another_string(): String {
    return "world"
}

fn main(): String {
    let result = select {
        get_string(),
        get_another_string()
    }
    return result
}
"#;

        let result = compile(code);

        assert!(
            result.is_ok(),
            "Select statement with matching types should compile"
        );
    }

    #[test]
    fn test_type_checker_integration_select_type_mismatch() {
        let code = r#"
fn get_string(): String {
    return "hello"
}

fn get_boolean(): Boolean {
    return true
}

fn main(): String {
    let result = select {
        get_string(),
        get_boolean()
    }
    return result
}
"#;

        let result = compile(code);

        assert!(
            result.is_err(),
            "Select returning a union type where String is declared should fail"
        );
        let err = result.unwrap_err();
        assert!(err.contains("mismatched return type"));
    }

    #[test]
    fn test_type_checker_integration_select_infers_union_type() {
        let code = r#"
fn get_string(): String {
    return "hello"
}

fn get_boolean(): Boolean {
    return true
}

fn main(): String | Boolean {
    return select {
        get_string(),
        get_boolean()
    }
}
"#;

        let result = compile(code);
        assert!(
            result.is_ok(),
            "Select with mixed arms should infer union type: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_type_checker_integration_external_function() {
        let code = r#"
extern fn validate_data(input: String): Boolean

fn main(): () {
    let is_valid = validate_data("test")
    if is_valid {
    }
}
"#;

        let result = compile(code);

        assert!(
            result.is_ok(),
            "External function should work with type checking"
        );
    }

    #[test]
    fn test_type_checker_integration_if_else_else_branch_type_error() {
        let code = r#"
fn main(): () {
    if true {
    } else {
        if "not a boolean" {
        }
    }
}
"#;

        let result = compile(code);

        if result.is_ok() {
            println!("Expected error but compilation succeeded");
        }
        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }

        assert!(
            result.is_err(),
            "If/else else-branch type error should fail compilation"
        );
        let err = result.unwrap_err();
        assert!(err.contains("type mismatch"));
        assert!(err.contains("expected `Boolean`, found `String`"));
    }

    #[test]
    fn test_generic_struct_instantiation_typechecks() {
        let code = r#"
struct Wrapper<T> {
    value: T,
}

fn main(): () {
    let w = Wrapper { value: "hello" }
}
"#;

        let result = compile(code);

        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(
            result.is_ok(),
            "Generic struct instantiation should compile"
        );
    }

    #[test]
    fn test_generic_struct_multiple_fields_same_type_param() {
        let code = r#"
struct Pair<T> {
    first: T,
    second: T,
}

fn main(): () {
    let p = Pair { first: "hello", second: "world" }
}
"#;

        let result = compile(code);

        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(
            result.is_ok(),
            "Generic struct with same type param on multiple fields should compile"
        );
    }

    #[test]
    fn test_generic_struct_field_type_mismatch_caught() {
        let code = r#"
struct Pair<T> {
    first: T,
    second: T,
}

fn main(): () {
    let p = Pair { first: "hello", second: 42 }
}
"#;

        let result = compile(code);

        assert!(
            result.is_err(),
            "Mismatched types on same type param should fail"
        );
        let err = result.unwrap_err();
        assert!(err.contains("type mismatch for field `second` of struct `Pair`"));
    }

    #[test]
    fn test_generic_struct_with_two_type_params() {
        let code = r#"
struct Either<A, B> {
    left: A,
    right: B,
}

fn main(): () {
    let e = Either { left: "hello", right: 42 }
}
"#;

        let result = compile(code);

        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(
            result.is_ok(),
            "Generic struct with two type params should compile"
        );
    }

    #[test]
    fn test_generic_function_call_typechecks_end_to_end() {
        let code = r#"
fn head<T>(list: List<T>): Option<T> {
    return head(list)
}

fn main(xs: List<String>): Option<String> {
    return head(xs)
}
"#;

        let result = compile(code);

        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(
            result.is_ok(),
            "Generic function call should compile successfully"
        );
    }

    #[test]
    fn test_constrained_generic_function_call_typechecks_end_to_end() {
        let code = r#"
fn some_fn<T: Int>(param: T): T {
    return param
}

fn main(): Int {
    let thing = some_fn(10)
    return thing
}
"#;

        let result = compile(code);

        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(
            result.is_ok(),
            "Constrained generic function call should compile successfully"
        );

        let runtime_result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                Runtime::builder(ProgramSource::Inline(code.to_string()))
                    .build()
                    .run(),
            )
            .unwrap();

        assert_eq!(runtime_result.as_integer().unwrap(), 10);
    }

    #[test]
    fn test_trait_dispatch_compiles() {
        let code = r#"
struct Container {
    value: Int,
}
trait Valuable {
    fn value_of(self: Self): Int
}
impl Container: Valuable {
    fn value_of(self: Container): Int {
        return self.value
    }
}
fn extract<T: Valuable>(x: T): Int {
    return x.value_of()
}
fn main(): Int {
    let c = Container { value: 42 }
    return extract(c)
}
"#;
        let result = compile(code);
        if let Err(ref e) = result {
            println!("Compilation error: {}", e);
        }
        assert!(
            result.is_ok(),
            "Trait dispatch should compile: {:?}",
            result
        );
    }

    #[test]
    fn test_trait_dispatch_runs() {
        let code = r#"
struct Container {
    value: Int,
}
trait Valuable {
    fn value_of(self: Self): Int
}
impl Container: Valuable {
    fn value_of(self: Container): Int {
        return self.value
    }
}
fn extract<T: Valuable>(x: T): Int {
    return x.value_of()
}
fn main(): Int {
    let c = Container { value: 42 }
    return extract(c)
}
"#;
        let result = compile(code);
        assert!(result.is_ok(), "Trait dispatch should compile");

        let runtime_result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                Runtime::builder(ProgramSource::Inline(code.to_string()))
                    .build()
                    .run(),
            )
            .unwrap();

        assert_eq!(runtime_result.as_integer().unwrap(), 42);
    }

    #[test]
    fn test_private_module_function_call_gives_clear_error() {
        let code = r#"
mod greet {
    fn hello(): String {
        "hello"
    }
}
use greet::hello
fn main(): () {
    hello()!
}
"#;

        let result = compile(code);
        assert!(
            result.is_err(),
            "Calling a private module function should fail to compile"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("private"),
            "Error should mention 'private', got: {}",
            err
        );
    }

    #[test]
    fn test_main_is_found_with_inline_modules() {
        let code = r#"
mod greet {
    pub fn hello(): String {
        return "hello"
    }
}
use greet::hello
fn main(): String {
    return hello()
}
"#;

        let result = compile(code);
        assert!(
            result.is_ok(),
            "Program with inline modules should compile: {:?}",
            result.err()
        );
        let compiled = result.unwrap();
        assert!(
            compiled.main_function_name().is_some(),
            "main function should be discoverable when inline modules are present"
        );
    }

    #[test]
    fn test_plan_impl_sample_main_is_found() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/plan-impl.sa");
        let result = Compiler::new()
            .with_module(std::sync::Arc::new(structured_agent_stdlib::io::IoModule))
            .with_module(std::sync::Arc::new(
                structured_agent_stdlib::messaging::MessagingModule,
            ))
            .with_module(std::sync::Arc::new(structured_agent_stdlib::fs::FsModule))
            .with_module(std::sync::Arc::new(
                structured_agent_stdlib::iterator::IteratorModule,
            ))
            .compile_file(path);
        assert!(
            result.is_ok(),
            "plan-impl.sa should compile: {:?}",
            result.err()
        );
        assert!(
            result.unwrap().main_function_name().is_some(),
            "main function should be discoverable in plan-impl.sa"
        );
    }
}
