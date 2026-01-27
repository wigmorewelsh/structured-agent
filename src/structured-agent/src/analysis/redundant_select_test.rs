#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, RedundantSelectAnalyzer};
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
    fn detects_single_branch_select() {
        let code = r#"
extern fn compute(): String

fn test(): () {
    let result = select {
        compute() as x => x
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = RedundantSelectAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_two_branches() {
        let code = r#"
extern fn option1(): String
extern fn option2(): String

fn test(): () {
    let result = select {
        option1() as x => x,
        option2() as y => y
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = RedundantSelectAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_multiple_branches() {
        let code = r#"
extern fn a(): String
extern fn b(): String
extern fn c(): String

fn test(): () {
    let result = select {
        a() as x => x,
        b() as y => y,
        c() as z => z
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = RedundantSelectAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_nested_single_branch_select() {
        let code = r#"
extern fn single(): String

fn test(): () {
    if true {
        let nested = select {
            single() as x => x
        }
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = RedundantSelectAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }
}
