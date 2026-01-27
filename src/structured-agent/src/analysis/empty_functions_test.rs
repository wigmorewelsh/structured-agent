#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, EmptyFunctionAnalyzer};
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
    fn detects_empty_function() {
        let code = r#"
fn empty(): () {
}
"#;

        let module = parse_code(code);
        let mut analyzer = EmptyFunctionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_function_with_statements() {
        let code = r#"
fn not_empty(): () {
    "has content"
}
"#;

        let module = parse_code(code);
        let mut analyzer = EmptyFunctionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_empty_functions() {
        let code = r#"
fn empty1(): () {
}

fn empty2(): () {
}

fn not_empty(): () {
    "content"
}
"#;

        let module = parse_code(code);
        let mut analyzer = EmptyFunctionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_function_with_return() {
        let code = r#"
fn returns_value(): String {
    return "result"
}
"#;

        let module = parse_code(code);
        let mut analyzer = EmptyFunctionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }
}
