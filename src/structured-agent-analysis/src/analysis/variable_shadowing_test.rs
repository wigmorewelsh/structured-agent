#[cfg(test)]
mod tests {
    use super::super::test_helpers::parse_code;
    use crate::analysis::{Analyzer, VariableShadowingAnalyzer};

    #[test]
    fn detects_parameter_shadowing() {
        let code = r#"
fn test(x: String): () {
    let x = "shadowing"
}
"#;

        let module = parse_code(code);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_nested_block_shadowing() {
        let code = r#"
fn test(): () {
    let x = "outer"
    if true {
        let x = "inner"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn detects_multiple_nested_scopes() {
        let code = r#"
fn test(): () {
    let x = "level1"
    if true {
        let x = "level2"
        while true {
            let x = "level3"
        }
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_different_variables() {
        let code = r#"
fn test(): () {
    let x = "outer"
    if true {
        let y = "inner"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_same_scope() {
        let code = r#"
fn test(): () {
    let x = "first"
    let y = "second"
}
"#;

        let module = parse_code(code);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_parameter_and_nested_shadowing() {
        let code = r#"
fn test(value: String): () {
    if true {
        let value = "shadowing"
    }
}
"#;

        let module = parse_code(code);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }
}
