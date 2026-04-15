#[cfg(test)]
mod tests {
    use crate::compiler::{CompilationUnit, CompiledProgram, Compiler};

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
        assert!(result.unwrap_err().contains("Type error"));
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
        assert!(err.contains("Type error"));
        assert!(err.contains("return type mismatch"));
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
        get_string() as s1 => s1,
        get_another_string() as s2 => s2
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
        get_string() as s => s,
        get_boolean() as b => b
    }
    return result
}
"#;

        let result = compile(code);

        assert!(
            result.is_err(),
            "Select statement with mismatched types should fail"
        );
        let err = result.unwrap_err();
        assert!(err.contains("Type error"));
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
        assert!(err.contains("Type error"));
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
        assert!(result.unwrap_err().contains("Type error"));
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
}
