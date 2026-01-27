#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, PlaceholderOveruseAnalyzer, Warning};
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
    fn detects_all_placeholder_arguments() {
        let func = create_test_function(
            "test",
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "process".to_string(),
                arguments: vec![
                    Expression::Placeholder {
                        span: Span::dummy(),
                    },
                    Expression::Placeholder {
                        span: Span::dummy(),
                    },
                    Expression::Placeholder {
                        span: Span::dummy(),
                    },
                ],
                span: Span::new(0, 20),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            warnings[0],
            Warning::PlaceholderOveruse {
                placeholder_count: 3,
                ..
            }
        ));
    }

    #[test]
    fn no_warning_for_mixed_arguments() {
        let func = create_test_function(
            "test",
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "process".to_string(),
                arguments: vec![
                    Expression::StringLiteral {
                        value: "concrete".to_string(),
                        span: Span::dummy(),
                    },
                    Expression::Placeholder {
                        span: Span::dummy(),
                    },
                    Expression::StringLiteral {
                        value: "value".to_string(),
                        span: Span::dummy(),
                    },
                ],
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_no_arguments() {
        let func = create_test_function(
            "test",
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "process".to_string(),
                arguments: vec![],
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_single_placeholder_argument() {
        let func = create_test_function(
            "test",
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "process".to_string(),
                arguments: vec![Expression::Placeholder {
                    span: Span::dummy(),
                }],
                span: Span::new(0, 10),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            warnings[0],
            Warning::PlaceholderOveruse {
                placeholder_count: 1,
                ..
            }
        ));
    }

    #[test]
    fn detects_nested_calls_with_placeholders() {
        let func = create_test_function(
            "test",
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![Statement::ExpressionStatement(Expression::Call {
                    function: "nested".to_string(),
                    arguments: vec![
                        Expression::Placeholder {
                            span: Span::dummy(),
                        },
                        Expression::Placeholder {
                            span: Span::dummy(),
                        },
                    ],
                    span: Span::new(10, 20),
                })],
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn no_warning_for_concrete_arguments() {
        let func = create_test_function(
            "test",
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "process".to_string(),
                arguments: vec![
                    Expression::StringLiteral {
                        value: "arg1".to_string(),
                        span: Span::dummy(),
                    },
                    Expression::StringLiteral {
                        value: "arg2".to_string(),
                        span: Span::dummy(),
                    },
                ],
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = PlaceholderOveruseAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }
}
