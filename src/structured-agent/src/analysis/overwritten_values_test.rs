#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, OverwrittenValueAnalyzer};
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
    fn detects_overwritten_value() {
        let code = r#"
fn test(): () {
    let x = "first"
    let x = "second"
}
"#;

        let module = parse_code(code);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_when_value_is_read() {
        let code = r#"
fn test(): () {
    let x = "first"
    x
    let x = "second"
}
"#;

        let module = parse_code(code);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_overwrite_in_expression() {
        let code = r#"
fn test(): () {
    let x = "first"
    let x = y
}
"#;

        let module = parse_code(code);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_different_variables() {
        let code = r#"
fn test(): () {
    let x = "first"
    let y = "second"
}
"#;

        let module = parse_code(code);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_read_in_nested_block() {
        let code = r#"
fn test(): () {
    let x = "first"
    if true {
        x
    }
    let x = "second"
}
"#;

        let module = parse_code(code);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_overwrites() {
        let code = r#"
fn test(): () {
    let x = "first"
    let x = "second"
    let x = "third"
}
"#;

        let module = parse_code(code);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }
}
