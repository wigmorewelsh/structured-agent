#[cfg(test)]
mod tests {
    use super::super::test_helpers::parse_code;
    use crate::analysis::{Analyzer, RedundantSelectAnalyzer};

    #[test]
    fn detects_single_branch_select() {
        let code = r#"
extern fn compute(): String

fn test(): () {
    let result = select {
        compute()
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
        option1(),
        option2()
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
        a(),
        b(),
        c()
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
            single()
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
