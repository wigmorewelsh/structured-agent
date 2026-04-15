#[cfg(test)]
mod tests {
    use super::super::test_helpers::parse_code;
    use crate::analysis::{Analyzer, PlaceholderOveruseAnalyzer};

    #[test]
    fn detects_all_placeholder_arguments() {
        let code = r#"
extern fn process(a: String, b: String, c: String): ()

fn test(): () {
    process(_, _, _)
}
"#;

        let module = parse_code(code);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_mixed_arguments() {
        let code = r#"
extern fn process(a: String, b: String, c: String): ()

fn test(): () {
    process("concrete", _, "value")
}
"#;

        let module = parse_code(code);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_no_arguments() {
        let code = r#"
extern fn process(): ()

fn test(): () {
    process()
}
"#;

        let module = parse_code(code);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_single_placeholder_argument() {
        let code = r#"
extern fn process(a: String): ()

fn test(): () {
    process(_)
}
"#;

        let module = parse_code(code);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_nested_calls_with_placeholders() {
        let code = r#"
extern fn nested(a: String, b: String): ()

fn test(): () {
    if true {
        nested(_, _)
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_concrete_arguments() {
        let code = r#"
extern fn process(a: String, b: String): ()

fn test(): () {
    process("arg1", "arg2")
}
"#;

        let module = parse_code(code);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }
}
