#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, EmptyBlockAnalyzer, Warning};
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
    fn detects_empty_if_block() {
        let func = create_test_function(
            "test",
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![],
                span: Span::new(0, 10),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = EmptyBlockAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::EmptyBlock { .. }));
    }

    #[test]
    fn detects_empty_while_block() {
        let func = create_test_function(
            "test",
            vec![Statement::While {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![],
                span: Span::new(0, 10),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = EmptyBlockAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::EmptyBlock { .. }));
    }

    #[test]
    fn no_warning_for_non_empty_blocks() {
        let func = create_test_function(
            "test",
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![Statement::Injection(Expression::StringLiteral {
                    value: "has content".to_string(),
                    span: Span::dummy(),
                })],
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = EmptyBlockAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_nested_empty_blocks() {
        let func = create_test_function(
            "test",
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![
                    Statement::Injection(Expression::StringLiteral {
                        value: "outer has content".to_string(),
                        span: Span::dummy(),
                    }),
                    Statement::While {
                        condition: Expression::BooleanLiteral {
                            value: false,
                            span: Span::dummy(),
                        },
                        body: vec![],
                        span: Span::new(20, 30),
                    },
                ],
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = EmptyBlockAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::EmptyBlock { .. }));
    }
}
