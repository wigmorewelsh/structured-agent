#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, UnusedReturnValueAnalyzer};
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
    fn detects_unused_return_value_from_external_function() {
        let code = r#"
extern fn get_data(): String

fn test(): () {
    get_data()
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_when_return_value_is_used() {
        let code = r#"
extern fn get_data(): String

fn test(): () {
    let result = get_data()
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_unit_return_type() {
        let code = r#"
extern fn log(): ()

fn test(): () {
    log()
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_unused_return_from_internal_function() {
        let code = r#"
fn get_value(): String {
    return "result"
}

fn test(): () {
    get_value()
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_when_used_in_injection() {
        let code = r#"
extern fn get_data(): String

fn test(): () {
    get_data()!
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_unused_returns() {
        let code = r#"
extern fn get_data1(): String
extern fn get_data2(): String

fn test(): () {
    get_data1()
    get_data2()
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }
}
