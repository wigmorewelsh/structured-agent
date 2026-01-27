#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, DuplicateInjectionAnalyzer};
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
    fn detects_duplicate_string_injections() {
        let code = r#"
fn test(): () {
    "message"!
    "message"!
}
"#;

        let module = parse_code(code);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_duplicate_variable_injections() {
        let code = r#"
fn test(): () {
    let status = "ready"
    status!
    status!
}
"#;

        let module = parse_code(code);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_different_injections() {
        let code = r#"
fn test(): () {
    "first"!
    "second"!
}
"#;

        let module = parse_code(code);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_consecutive_duplicates() {
        let code = r#"
fn test(): () {
    "message"!
    "message"!
    "message"!
}
"#;

        let module = parse_code(code);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn resets_tracking_after_control_flow() {
        let code = r#"
fn test(): () {
    "message"!
    if true {
        "different"!
    }
    "message"!
}
"#;

        let module = parse_code(code);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_complex_expressions() {
        let code = r#"
extern fn compute(): String

fn test(): () {
    compute()
    compute()
}
"#;

        let module = parse_code(code);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }
}
