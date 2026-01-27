#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, OverwrittenValueAnalyzer, Warning};
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
    fn detects_overwritten_value() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "first".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "second".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(20, 30),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        if let Warning::OverwrittenValue { name, .. } = &warnings[0] {
            assert_eq!(name, "x");
        } else {
            panic!("Expected OverwrittenValue warning");
        }
    }

    #[test]
    fn no_warning_when_value_is_read() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "first".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::Injection(Expression::Variable {
                    name: "x".to_string(),
                    span: Span::dummy(),
                }),
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "second".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(20, 30),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_overwrite_in_expression() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "first".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::Variable {
                        name: "y".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(20, 30),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        if let Warning::OverwrittenValue { name, .. } = &warnings[0] {
            assert_eq!(name, "x");
        } else {
            panic!("Expected OverwrittenValue warning");
        }
    }

    #[test]
    fn no_warning_for_different_variables() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "first".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::Assignment {
                    variable: "y".to_string(),
                    expression: Expression::StringLiteral {
                        value: "second".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(20, 30),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_read_in_nested_block() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "first".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::Variable {
                        name: "x".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::dummy(),
                },
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "second".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(30, 40),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_overwrites() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "first".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "second".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(20, 30),
                },
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "third".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(40, 50),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = OverwrittenValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }
}
