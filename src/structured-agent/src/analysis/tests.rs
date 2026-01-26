#[cfg(test)]
mod tests {
    use crate::analysis::{
        Analyzer, InfiniteLoopAnalyzer, ReachabilityAnalyzer, UnusedVariableAnalyzer, Warning,
    };
    use crate::ast::{
        Definition, Expression, Function, FunctionBody, Module, Parameter, Statement, Type,
    };
    use crate::types::{FileId, Span};

    fn create_test_function(
        name: &str,
        parameters: Vec<Parameter>,
        return_type: Type,
        statements: Vec<Statement>,
    ) -> Function {
        Function {
            name: name.to_string(),
            parameters,
            return_type,
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
    fn test_unused_variable_detected() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![Statement::Assignment {
                variable: "unused".to_string(),
                expression: Expression::StringLiteral {
                    value: "hello".to_string(),
                    span: Span::new(10, 17),
                },
                span: Span::new(0, 17),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = UnusedVariableAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            Warning::UnusedVariable { name, .. } => {
                assert_eq!(name, "unused");
            }
            _ => panic!("Expected UnusedVariable warning"),
        }
    }

    #[test]
    fn test_used_variable_no_warning() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![
                Statement::Assignment {
                    variable: "used".to_string(),
                    expression: Expression::StringLiteral {
                        value: "hello".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::Injection(Expression::Variable {
                    name: "used".to_string(),
                    span: Span::dummy(),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = UnusedVariableAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_unused_parameter_detected() {
        let func = create_test_function(
            "test",
            vec![Parameter {
                name: "param".to_string(),
                param_type: Type::Named("String".to_string()),
                span: Span::new(10, 15),
            }],
            Type::Unit,
            vec![Statement::Injection(Expression::StringLiteral {
                value: "hello".to_string(),
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = UnusedVariableAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            Warning::UnusedVariable { name, .. } => {
                assert_eq!(name, "param");
            }
            _ => panic!("Expected UnusedVariable warning"),
        }
    }

    #[test]
    fn test_used_parameter_no_warning() {
        let func = create_test_function(
            "test",
            vec![Parameter {
                name: "param".to_string(),
                param_type: Type::Named("String".to_string()),
                span: Span::dummy(),
            }],
            Type::Unit,
            vec![Statement::Injection(Expression::Variable {
                name: "param".to_string(),
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = UnusedVariableAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_variable_in_nested_scope() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![
                Statement::Assignment {
                    variable: "used".to_string(),
                    expression: Expression::StringLiteral {
                        value: "hello".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::Variable {
                        name: "used".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::dummy(),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = UnusedVariableAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_analysis_runner_collects_all_warnings() {
        use crate::analysis::AnalysisRunner;

        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![
                Statement::Assignment {
                    variable: "unused".to_string(),
                    expression: Expression::StringLiteral {
                        value: "hello".to_string(),
                        span: Span::new(0, 5),
                    },
                    span: Span::new(0, 5),
                },
                Statement::While {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::StringLiteral {
                        value: "loop".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::new(10, 20),
                },
                Statement::Injection(Expression::StringLiteral {
                    value: "unreachable".to_string(),
                    span: Span::new(30, 40),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut runner = AnalysisRunner::new()
            .with_analyzer(Box::new(UnusedVariableAnalyzer::new()))
            .with_analyzer(Box::new(ReachabilityAnalyzer::new()))
            .with_analyzer(Box::new(InfiniteLoopAnalyzer::new()));

        let warnings = runner.run(&module, 0);

        assert_eq!(warnings.len(), 3);

        let has_unused = warnings
            .iter()
            .any(|w| matches!(w, Warning::UnusedVariable { .. }));
        let has_unreachable = warnings
            .iter()
            .any(|w| matches!(w, Warning::UnreachableCode { .. }));
        let has_infinite = warnings
            .iter()
            .any(|w| matches!(w, Warning::PotentialInfiniteLoop { .. }));

        assert!(has_unused);
        assert!(has_unreachable);
        assert!(has_infinite);
    }

    #[test]
    fn test_unreachable_code_after_return() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![
                Statement::Return(Expression::StringLiteral {
                    value: "early".to_string(),
                    span: Span::new(0, 5),
                }),
                Statement::Injection(Expression::StringLiteral {
                    value: "unreachable".to_string(),
                    span: Span::new(10, 20),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ReachabilityAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            Warning::UnreachableCode { span, .. } => {
                assert_eq!(span.start, 10);
                assert_eq!(span.end, 20);
            }
            _ => panic!("Expected UnreachableCode warning"),
        }
    }

    #[test]
    fn test_no_unreachable_code() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![
                Statement::Injection(Expression::StringLiteral {
                    value: "hello".to_string(),
                    span: Span::dummy(),
                }),
                Statement::Injection(Expression::StringLiteral {
                    value: "world".to_string(),
                    span: Span::dummy(),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ReachabilityAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_unreachable_after_infinite_loop() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![
                Statement::While {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::StringLiteral {
                        value: "looping".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::new(0, 10),
                },
                Statement::Injection(Expression::StringLiteral {
                    value: "unreachable".to_string(),
                    span: Span::new(20, 30),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = ReachabilityAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            Warning::UnreachableCode { span, .. } => {
                assert_eq!(span.start, 20);
                assert_eq!(span.end, 30);
            }
            _ => panic!("Expected UnreachableCode warning"),
        }
    }

    #[test]
    fn test_infinite_loop_detected() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![Statement::While {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![Statement::Injection(Expression::StringLiteral {
                    value: "forever".to_string(),
                    span: Span::dummy(),
                })],
                span: Span::new(5, 15),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = InfiniteLoopAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            Warning::PotentialInfiniteLoop { span, .. } => {
                assert_eq!(span.start, 5);
                assert_eq!(span.end, 15);
            }
            _ => panic!("Expected PotentialInfiniteLoop warning"),
        }
    }

    #[test]
    fn test_infinite_loop_with_return_no_warning() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![Statement::While {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![Statement::Return(Expression::StringLiteral {
                    value: "escape".to_string(),
                    span: Span::dummy(),
                })],
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = InfiniteLoopAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_finite_loop_no_warning() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![Statement::While {
                condition: Expression::Variable {
                    name: "condition".to_string(),
                    span: Span::dummy(),
                },
                body: vec![Statement::Injection(Expression::StringLiteral {
                    value: "maybe".to_string(),
                    span: Span::dummy(),
                })],
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = InfiniteLoopAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_variable_infinite_loop_detected() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![
                Statement::Assignment {
                    variable: "continue_loop".to_string(),
                    expression: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::While {
                    condition: Expression::Variable {
                        name: "continue_loop".to_string(),
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Injection(Expression::StringLiteral {
                        value: "forever".to_string(),
                        span: Span::dummy(),
                    })],
                    span: Span::new(10, 20),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = InfiniteLoopAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            Warning::PotentialInfiniteLoop { span, .. } => {
                assert_eq!(span.start, 10);
                assert_eq!(span.end, 20);
            }
            _ => panic!("Expected PotentialInfiniteLoop warning"),
        }
    }

    #[test]
    fn test_variable_modified_in_loop_no_warning() {
        let func = create_test_function(
            "test",
            vec![],
            Type::Unit,
            vec![
                Statement::Assignment {
                    variable: "active".to_string(),
                    expression: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    span: Span::dummy(),
                },
                Statement::While {
                    condition: Expression::Variable {
                        name: "active".to_string(),
                        span: Span::dummy(),
                    },
                    body: vec![Statement::VariableAssignment {
                        variable: "active".to_string(),
                        expression: Expression::BooleanLiteral {
                            value: false,
                            span: Span::dummy(),
                        },
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = InfiniteLoopAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }
}
