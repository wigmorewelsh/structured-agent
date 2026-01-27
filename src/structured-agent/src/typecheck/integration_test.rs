#[cfg(test)]
mod tests {
    use crate::compiler::{CompilationUnit, Compiler, CompilerTrait};

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

        let unit = CompilationUnit::from_string(code.to_string());
        let compiler = Compiler::new();
        let result = compiler.compile_program(&unit);

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

        let unit = CompilationUnit::from_string(code.to_string());
        let compiler = Compiler::new();
        let result = compiler.compile_program(&unit);

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

        let unit = CompilationUnit::from_string(code.to_string());
        let compiler = Compiler::new();
        let result = compiler.compile_program(&unit);

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

        let unit = CompilationUnit::from_string(code.to_string());
        let compiler = Compiler::new();
        let result = compiler.compile_program(&unit);

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

        let unit = CompilationUnit::from_string(code.to_string());
        let compiler = Compiler::new();
        let result = compiler.compile_program(&unit);

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

        let unit = CompilationUnit::from_string(code.to_string());
        let compiler = Compiler::new();
        let result = compiler.compile_program(&unit);

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

        let unit = CompilationUnit::from_string(code.to_string());
        let compiler = Compiler::new();
        let result = compiler.compile_program(&unit);

        assert!(
            result.is_ok(),
            "External function should work with type checking"
        );
    }
}
