#[cfg(test)]
mod tests {
    use super::super::test_helpers::parse_code;
    use crate::analysis::{Analyzer, OrphanedInjectionAnalyzer};

    #[test]
    fn injection_last_in_if_body() {
        let code = r#"
extern fn get_context(): String

fn test(): () {
    if true {
        get_context()!
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn injection_last_in_else_body() {
        let code = r#"
extern fn get_context(): String
extern fn use_context(ctx: String): ()

fn test(): () {
    if true {
        use_context("x")
    } else {
        get_context()!
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn injection_last_in_both_branches() {
        let code = r#"
extern fn get_context(): String

fn test(): () {
    if true {
        get_context()!
    } else {
        get_context()!
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn injection_last_in_while_body() {
        let code = r#"
extern fn get_context(): String

fn test(): () {
    while true {
        get_context()!
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn injection_last_in_for_body() {
        let code = r#"
extern fn get_context(): String

fn test(): () {
    for item in items {
        get_context()!
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn injection_not_last_in_if_body() {
        let code = r#"
extern fn get_context(): String
extern fn use_context(ctx: String): ()

fn test(): () {
    if true {
        get_context()!
        use_context("x")
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn injection_at_top_level_function_body() {
        let code = r#"
extern fn get_context(): String

fn test(): () {
    get_context()!
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_injections_in_blocks() {
        let code = r#"
extern fn compute(): String
extern fn use_it(s: String): ()

fn test(): () {
    if true {
        let x = compute()
        use_it(x)
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn injection_last_in_nested_if() {
        let code = r#"
extern fn get_context(): String

fn test(): () {
    if true {
        if true {
            get_context()!
        }
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = OrphanedInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 1);
    }
}
