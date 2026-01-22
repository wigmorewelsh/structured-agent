use crate::compiler::parser::parse_program;
use combine::EasyParser;

#[test]
fn test_comment_documentation_parsing() {
    let input = r#"
# This is a documented function
# It demonstrates multi-line comments
fn documented_func(): () {
    "Hello"!
}

fn undocumented_func(): () {
    "World"!
}
"#;

    let result = parse_program().easy_parse(input);
    assert!(result.is_ok());

    let (module, _) = result.unwrap();
    let functions: Vec<_> = module
        .definitions
        .iter()
        .filter_map(|def| match def {
            crate::ast::Definition::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(functions.len(), 2);

    // Check documented function
    let documented = functions[0];
    assert_eq!(documented.name, "documented_func");
    assert!(documented.documentation.is_some());
    let doc = documented.documentation.as_ref().unwrap();
    assert_eq!(
        doc,
        "This is a documented function\nIt demonstrates multi-line comments"
    );

    // Check undocumented function
    let undocumented = functions[1];
    assert_eq!(undocumented.name, "undocumented_func");
    assert!(undocumented.documentation.is_none());
}

#[test]
fn test_single_line_comment_parsing() {
    let input = r#"
# Single line doc
fn single_doc(): () {
    "Test"!
}
"#;

    let result = parse_program().easy_parse(input);
    assert!(result.is_ok());

    let (module, _) = result.unwrap();
    let func = match &module.definitions[0] {
        crate::ast::Definition::Function(f) => f,
        _ => panic!("Expected function definition"),
    };
    assert_eq!(func.name, "single_doc");
    assert!(func.documentation.is_some());
    let doc = func.documentation.as_ref().unwrap();
    assert_eq!(doc, "Single line doc");
}

#[test]
fn test_comment_display() {
    use std::fmt::Write;

    let input = r#"
# This function greets users
# It takes a name parameter
fn greet(name: String): () {
    "Hello"!
    name!
}
"#;

    let result = parse_program().easy_parse(input);
    assert!(result.is_ok());

    let (module, _) = result.unwrap();
    let func = match &module.definitions[0] {
        crate::ast::Definition::Function(f) => f,
        _ => panic!("Expected function definition"),
    };

    let mut output = String::new();
    write!(&mut output, "{}", func).unwrap();

    assert!(output.contains("# This function greets users"));
    assert!(output.contains("# It takes a name parameter"));
    assert!(output.contains("fn greet(name: String): ()"));
}
