#[cfg(test)]
mod tests {
    use super::super::test_helpers::parse_code;
    use crate::analysis::{Analyzer, MissingReturnOrInjectionAnalyzer};

    #[test]
    fn warns_when_no_return_or_injection() {
        let code = r#"
fn f(): Int {
    let x = 1
}
"#;
        let module = parse_code(code);
        let mut analyzer = MissingReturnOrInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_when_return_present() {
        let code = r#"
fn f(): Int {
    return 1
}
"#;
        let module = parse_code(code);
        let mut analyzer = MissingReturnOrInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_when_injection_present() {
        let code = r#"
fn f(): String {
    "hello"!
}
"#;
        let module = parse_code(code);
        let mut analyzer = MissingReturnOrInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_when_return_inside_if() {
        let code = r#"
fn f(): () {
    if true {
        return ()
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = MissingReturnOrInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn warns_for_impl_method_with_no_return() {
        let code = r#"
struct Foo {
    x: Int,
}
impl Foo {
    pub fn get(self): Int {
        self.x
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = MissingReturnOrInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_impl_method_with_return() {
        let code = r#"
struct Foo {
    x: Int,
}
impl Foo {
    pub fn get(self): Int {
        return self.x
    }
}
"#;
        let module = parse_code(code);
        let mut analyzer = MissingReturnOrInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);
        assert_eq!(warnings.len(), 0);
    }
}
