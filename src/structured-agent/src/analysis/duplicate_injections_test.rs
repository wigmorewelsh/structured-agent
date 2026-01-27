#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, DuplicateInjectionAnalyzer, Warning};
    use crate::ast::{Definition, Expression, Function, FunctionBody, Module, Statement, Type};
    use crate::types::Span;

    fn create_test_function(name: &str, statements: Vec<Statement>) -> Function {
        Function {
            name: name.to_string(),
            parameters: vec![],
            return_type: Type::Unit,
            body: FunctionBody {
                statements,
                span: Span::dummy(),
            },
            documentation: None,
            span: Span::dummy(),
        }
    }

    fn create_test_module(definitions: Vec<Definition>) -> Module {
        Module {
            definitions,
            span: Span::dummy(),
            file_id: 0,
        }
    }

    #[test]
    fn detects_duplicate_string_injections() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Injection(Expression::StringLiteral {
                    value: "message".to_string(),
                    span: Span::new(0, 9),
                }),
                Statement::Injection(Expression::StringLiteral {
                    value: "message".to_string(),
                    span: Span::new(10, 19),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::DuplicateInjection { .. }));
    }

    #[test]
    fn detects_duplicate_variable_injections() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "status".to_string(),
                    expression: Expression::StringLiteral {
                        value: "ready".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::Injection(Expression::Variable {
                    name: "status".to_string(),
                    span: Span::new(0, 6),
                }),
                Statement::Injection(Expression::Variable {
                    name: "status".to_string(),
                    span: Span::new(7, 13),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::DuplicateInjection { .. }));
    }

    #[test]
    fn no_warning_for_different_injections() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Injection(Expression::StringLiteral {
                    value: "first".to_string(),
                    span: Span::dummy(),
                }),
                Statement::Injection(Expression::StringLiteral {
                    value: "second".to_string(),
                    span: Span::dummy(),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_consecutive_duplicates() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Injection(Expression::StringLiteral {
                    value: "message".to_string(),
                    span: Span::new(0, 9),
                }),
                Statement::Injection(Expression::StringLiteral {
                    value: "message".to_string(),
                    span: Span::new(10, 19),
                }),
                Statement::Injection(Expression::StringLiteral {
                    value: "message".to_string(),
                    span: Span::new(20, 29),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn resets_tracking_after_control_flow() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Injection(Expression::StringLiteral {
                    value: "message".to_string(),
                    span: Span::dummy(),
                }),
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::StringLiteral {
                        value: "different".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::dummy(),
                },
                Statement::Injection(Expression::StringLiteral {
                    value: "message".to_string(),
                    span: Span::dummy(),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_complex_expressions() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Injection(Expression::Call {
                    function: "compute".to_string(),
                    arguments: vec![],
                    span: Span::dummy(),
                }),
                Statement::Injection(Expression::Call {
                    function: "compute".to_string(),
                    arguments: vec![],
                    span: Span::dummy(),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = DuplicateInjectionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }
}
