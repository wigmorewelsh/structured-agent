#[cfg(test)]
mod tests {
    use super::super::test_helpers::parse_code;
    use crate::analysis::{Analyzer, UnusedVariableAnalyzer, Warning};

    #[test]
    fn no_warning_when_variable_used_only_in_interpolation() {
        let code = r#"
fn test(): String {
    let name = "world"
    return "hello ${name}"
}
"#;

        let func = parse_code(code);
        let mut analyzer = UnusedVariableAnalyzer::new();
        let warnings = analyzer.analyze_function(
            match &func.definitions[0] {
                structured_agent_ast::ast::Definition::Function(f) => f,
                _ => panic!("expected function"),
            },
            0,
        );

        let unused: Vec<_> = warnings
            .iter()
            .filter(|w| matches!(w, Warning::UnusedVariable { .. }))
            .collect();
        assert_eq!(unused.len(), 0);
    }

    #[test]
    fn warns_when_variable_truly_unused_alongside_template() {
        let code = r#"
fn test(): String {
    let name = "world"
    let unused_var = "ignored"
    return "hello ${name}"
}
"#;

        let func = parse_code(code);
        let mut analyzer = UnusedVariableAnalyzer::new();
        let warnings = analyzer.analyze_function(
            match &func.definitions[0] {
                structured_agent_ast::ast::Definition::Function(f) => f,
                _ => panic!("expected function"),
            },
            0,
        );

        let unused: Vec<_> = warnings
            .iter()
            .filter(|w| matches!(w, Warning::UnusedVariable { .. }))
            .collect();
        assert_eq!(unused.len(), 1);
        assert!(matches!(&unused[0], Warning::UnusedVariable { name, .. } if name == "unused_var"));
    }
}
