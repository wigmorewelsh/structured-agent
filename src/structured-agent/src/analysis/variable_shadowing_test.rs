#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, VariableShadowingAnalyzer, Warning};
    use crate::ast::{
        Definition, Expression, Function, FunctionBody, Module, Parameter, Statement, Type,
    };
    use crate::types::Span;

    fn create_test_function(
        name: &str,
        parameters: Vec<Parameter>,
        statements: Vec<Statement>,
    ) -> Function {
        Function {
            name: name.to_string(),
            parameters,
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
    fn detects_parameter_shadowing() {
        let param = Parameter {
            name: "x".to_string(),
            param_type: Type::Named("String".to_string()),
            span: Span::new(0, 5),
        };

        let func = create_test_function(
            "test",
            vec![param],
            vec![Statement::Assignment {
                variable: "x".to_string(),
                expression: Expression::StringLiteral {
                    value: "shadowing".to_string(),
                    span: Span::dummy(),
                },
                span: Span::new(10, 20),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        if let Warning::VariableShadowing { name, .. } = &warnings[0] {
            assert_eq!(name, "x");
        } else {
            panic!("Expected VariableShadowing warning");
        }
    }

    #[test]
    fn detects_nested_block_shadowing() {
        let func = create_test_function(
            "test",
            vec![],
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "outer".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Assignment {
                        variable: "x".to_string(),
                        expression: Expression::StringLiteral {
                            value: "inner".to_string(),
                            span: Span::dummy(),
                        },
                        span: Span::new(20, 30),
                    }],
                    span: Span::dummy(),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        if let Warning::VariableShadowing { name, .. } = &warnings[0] {
            assert_eq!(name, "x");
        } else {
            panic!("Expected VariableShadowing warning");
        }
    }

    #[test]
    fn detects_multiple_nested_scopes() {
        let func = create_test_function(
            "test",
            vec![],
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "level1".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![
                        Statement::Assignment {
                            variable: "x".to_string(),
                            expression: Expression::StringLiteral {
                                value: "level2".to_string(),
                                span: Span::dummy(),
                            },
                            span: Span::new(20, 30),
                        },
                        Statement::While {
                            condition: Expression::BooleanLiteral {
                                value: true,
                                span: Span::dummy(),
                            },
                            body: vec![Statement::Assignment {
                                variable: "x".to_string(),
                                expression: Expression::StringLiteral {
                                    value: "level3".to_string(),
                                    span: Span::dummy(),
                                },
                                span: Span::new(40, 50),
                            }],
                            span: Span::dummy(),
                        },
                    ],
                    span: Span::dummy(),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_different_variables() {
        let func = create_test_function(
            "test",
            vec![],
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral {
                        value: "outer".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(0, 10),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: Span::dummy(),
                    },
                    body: vec![Statement::Assignment {
                        variable: "y".to_string(),
                        expression: Expression::StringLiteral {
                            value: "inner".to_string(),
                            span: Span::dummy(),
                        },
                        span: Span::new(20, 30),
                    }],
                    span: Span::dummy(),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_same_scope() {
        let func = create_test_function(
            "test",
            vec![],
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
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_parameter_and_nested_shadowing() {
        let param = Parameter {
            name: "value".to_string(),
            param_type: Type::Named("String".to_string()),
            span: Span::new(0, 5),
        };

        let func = create_test_function(
            "test",
            vec![param],
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![Statement::Assignment {
                    variable: "value".to_string(),
                    expression: Expression::StringLiteral {
                        value: "shadowing".to_string(),
                        span: Span::dummy(),
                    },
                    span: Span::new(20, 30),
                }],
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = VariableShadowingAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        if let Warning::VariableShadowing { name, .. } = &warnings[0] {
            assert_eq!(name, "value");
        } else {
            panic!("Expected VariableShadowing warning");
        }
    }
}
