#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, ConstantConditionAnalyzer, Warning};
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
    fn detects_literal_true_condition() {
        let func = create_test_function(
            "test",
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![Statement::Injection(Expression::StringLiteral {
                    value: "always".to_string(),
                    span: Span::dummy(),
                })],
                span: Span::new(0, 10),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            warnings[0],
            Warning::ConstantCondition {
                condition_value: true,
                ..
            }
        ));
    }

    #[test]
    fn detects_literal_false_condition() {
        let func = create_test_function(
            "test",
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: false,
                    span: Span::dummy(),
                },
                body: vec![Statement::Injection(Expression::StringLiteral {
                    value: "never".to_string(),
                    span: Span::dummy(),
                })],
                span: Span::new(0, 10),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            warnings[0],
            Warning::ConstantCondition {
                condition_value: false,
                ..
            }
        ));
    }

    #[test]
    fn detects_variable_assigned_to_true() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "always_true".to_string(),
                    expression: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::If {
                    condition: Expression::Variable {
                        name: "always_true".to_string(),
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::StringLiteral {
                        value: "constant".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::new(10, 20),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            warnings[0],
            Warning::ConstantCondition {
                condition_value: true,
                ..
            }
        ));
    }

    #[test]
    fn no_warning_for_non_constant_variable() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "computed".to_string(),
                    expression: Expression::Call {
                        function: "compute".to_string(),
                        arguments: vec![],
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::If {
                    condition: Expression::Variable {
                        name: "computed".to_string(),
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::StringLiteral {
                        value: "maybe".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::dummy(),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_after_variable_reassignment() {
        let func = create_test_function(
            "test",
            vec![
                Statement::Assignment {
                    variable: "value".to_string(),
                    expression: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::VariableAssignment {
                    variable: "value".to_string(),
                    expression: Expression::Call {
                        function: "compute".to_string(),
                        arguments: vec![],
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::If {
                    condition: Expression::Variable {
                        name: "value".to_string(),
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::StringLiteral {
                        value: "maybe".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::dummy(),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_constant_conditions() {
        let func = create_test_function(
            "test",
            vec![
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![],
                    span: Span::new(0, 10),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: false,
                        span: Span::dummy(),
                    },
                    body: vec![],
                    span: Span::new(11, 21),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn detects_constant_condition_in_if_else_expression() {
        let func = create_test_function(
            "test",
            vec![Statement::Assignment {
                variable: "result".to_string(),
                expression: Expression::IfElse {
                    condition: Box::new(Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    }),
                    then_expr: Box::new(Expression::StringLiteral {
                        value: "always this".to_string(),
                        span: Span::dummy(),
                    }),
                    else_expr: Box::new(Expression::StringLiteral {
                        value: "never this".to_string(),
                        span: Span::dummy(),
                    }),
                    span: Span::dummy(),
                },
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            warnings[0],
            Warning::ConstantCondition {
                condition_value: true,
                ..
            }
        ));
    }

    #[test]
    fn detects_constant_condition_in_nested_if_else() {
        let func = create_test_function(
            "test",
            vec![Statement::Return(Expression::IfElse {
                condition: Box::new(Expression::BooleanLiteral {
                    value: false,
                    span: Span::dummy(),
                }),
                then_expr: Box::new(Expression::StringLiteral {
                    value: "outer then".to_string(),
                    span: Span::dummy(),
                }),
                else_expr: Box::new(Expression::IfElse {
                    condition: Box::new(Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    }),
                    then_expr: Box::new(Expression::StringLiteral {
                        value: "inner then".to_string(),
                        span: Span::dummy(),
                    }),
                    else_expr: Box::new(Expression::StringLiteral {
                        value: "inner else".to_string(),
                        span: Span::dummy(),
                    }),
                    span: Span::dummy(),
                }),
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ConstantConditionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }
}
