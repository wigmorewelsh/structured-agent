#[cfg(test)]
mod tests {
    use super::super::test_helpers::parse_code;
    use crate::analysis::{Analyzer, UnusedExpressionAnalyzer};

    #[test]
    fn detects_unused_string_literal() {
        let code = r#"
fn test(): () {
    "unused string"
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_unused_multiline_string() {
        let code = r#"
fn test(): () {
    '''
    This is an unused
    multiline string
    '''
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_when_string_is_injected() {
        let code = r#"
fn test(): () {
    "injected string"!
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_when_string_is_assigned() {
        let code = r#"
fn test(): () {
    let message = "stored string"
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_unused_boolean_literal() {
        let code = r#"
fn test(): () {
    true
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_unused_list_literal() {
        let code = r#"
fn test(): () {
    ["item1", "item2"]
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_unused_unit_literal() {
        let code = r#"
fn test(): () {
    ()
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_multiple_unused_expressions() {
        let code = r#"
fn test(): () {
    "first"
    "second"
    true
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 3);
    }

    #[test]
    fn no_warning_for_function_calls() {
        let code = r#"
extern fn do_something(): ()

fn test(): () {
    do_something()
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_unused_in_nested_blocks() {
        let code = r#"
fn test(): () {
    if true {
        "unused in then block"
    } else {
        "unused in else block"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn detects_unused_in_while_loop() {
        let code = r#"
fn test(): () {
    while true {
        "unused in loop"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_when_returned() {
        let code = r#"
fn test(): String {
    return "returned value"
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn mixed_used_and_unused() {
        let code = r#"
fn test(): () {
    "unused literal"
    let used = "assigned value"
    "another unused"!
    true
}
"#;

        let module = parse_code(code);
        let mut analyzer = UnusedExpressionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }
}
