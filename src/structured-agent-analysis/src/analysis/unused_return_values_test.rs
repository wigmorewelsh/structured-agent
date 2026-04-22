#[cfg(test)]
mod tests {
    use super::super::test_helpers::parse_code;
    use crate::analysis::{Analyzer, UnusedReturnValueAnalyzer};

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
