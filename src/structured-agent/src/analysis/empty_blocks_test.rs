#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, EmptyBlockAnalyzer};
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
    fn detects_empty_if_block() {
        let code = r#"
fn test(): () {
    if true {
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = EmptyBlockAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_empty_while_block() {
        let code = r#"
fn test(): () {
    while true {
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = EmptyBlockAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_non_empty_blocks() {
        let code = r#"
fn test(): () {
    if true {
        "has content"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = EmptyBlockAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_nested_empty_blocks() {
        let code = r#"
fn test(): () {
    if true {
        "outer has content"
        while false {
        }
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = EmptyBlockAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }
}
