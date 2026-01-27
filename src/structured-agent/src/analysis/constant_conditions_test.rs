#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, ConstantConditionAnalyzer};
    use crate::ast::Module;
    use crate::compiler::{CodespanParser, CompilationUnit, Parser};
    use crate::diagnostics::DiagnosticManager;

    fn parse_code(code: &str) -> Module {
        let unit = CompilationUnit::from_string(code.to_string());
        let manager = DiagnosticManager::new();
        let parser = CodespanParser::new(manager.reporter().clone());
        parser.parse(&unit, 0, manager.reporter()).unwrap()
    }

    #[test]
    fn detects_literal_true_condition() {
        let code = r#"
fn test(): () {
    if true {
        "always"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_literal_false_condition() {
        let code = r#"
fn test(): () {
    if false {
        "never"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_variable_assigned_to_true() {
        let code = r#"
fn test(): () {
    let always_true = true
    if always_true {
        "constant"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_non_constant_variable() {
        let code = r#"
extern fn compute(): Boolean

fn test(): () {
    let computed = compute()
    if computed {
        "maybe"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_after_variable_reassignment() {
        let code = r#"
extern fn compute(): Boolean

fn test(): () {
    let value = true
    value = compute()
    if value {
        "maybe"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_constant_conditions() {
        let code = r#"
fn test(): () {
    if true {
    }
    if false {
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn detects_constant_condition_in_if_else_expression() {
        let code = r#"
fn test(): () {
    let result = if true { "always this" } else { "never this" }
}
"#;

        let module = parse_code(code);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_constant_condition_in_nested_if_else() {
        let code = r#"
fn test(): String {
    return if false { "outer then" } else { if true { "inner then" } else { "inner else" } }
}
"#;

        let module = parse_code(code);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }
}
