#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, RedundantSelectAnalyzer, Warning};
    use crate::ast::{
        Definition, Expression, Function, FunctionBody, Module, SelectClause, SelectExpression,
        Statement, Type,
    };
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
    fn detects_single_branch_select() {
        let func = create_test_function(
            "test",
            vec![Statement::Assignment {
                variable: "result".to_string(),
                expression: Expression::Select(SelectExpression {
                    clauses: vec![SelectClause {
                        expression_to_run: Expression::Call {
                            function: "compute".to_string(),
                            arguments: vec![],
                            span: Span::dummy(),
                        },
                        result_variable: "x".to_string(),
                        expression_next: Expression::Variable {
                            name: "x".to_string(),
                            span: Span::dummy(),
                        },
                        span: Span::dummy(),
                    }],
                    span: Span::new(0, 20),
                }),
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = RedundantSelectAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::RedundantSelect { .. }));
    }

    #[test]
    fn no_warning_for_two_branches() {
        let func = create_test_function(
            "test",
            vec![Statement::Assignment {
                variable: "result".to_string(),
                expression: Expression::Select(SelectExpression {
                    clauses: vec![
                        SelectClause {
                            expression_to_run: Expression::Call {
                                function: "option1".to_string(),
                                arguments: vec![],
                                span: Span::dummy(),
                            },
                            result_variable: "x".to_string(),
                            expression_next: Expression::Variable {
                                name: "x".to_string(),
                                span: Span::dummy(),
                            },
                            span: Span::dummy(),
                        },
                        SelectClause {
                            expression_to_run: Expression::Call {
                                function: "option2".to_string(),
                                arguments: vec![],
                                span: Span::dummy(),
                            },
                            result_variable: "y".to_string(),
                            expression_next: Expression::Variable {
                                name: "y".to_string(),
                                span: Span::dummy(),
                            },
                            span: Span::dummy(),
                        },
                    ],
                    span: Span::dummy(),
                }),
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = RedundantSelectAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_multiple_branches() {
        let func = create_test_function(
            "test",
            vec![Statement::Assignment {
                variable: "result".to_string(),
                expression: Expression::Select(SelectExpression {
                    clauses: vec![
                        SelectClause {
                            expression_to_run: Expression::Call {
                                function: "a".to_string(),
                                arguments: vec![],
                                span: Span::dummy(),
                            },
                            result_variable: "x".to_string(),
                            expression_next: Expression::Variable {
                                name: "x".to_string(),
                                span: Span::dummy(),
                            },
                            span: Span::dummy(),
                        },
                        SelectClause {
                            expression_to_run: Expression::Call {
                                function: "b".to_string(),
                                arguments: vec![],
                                span: Span::dummy(),
                            },
                            result_variable: "y".to_string(),
                            expression_next: Expression::Variable {
                                name: "y".to_string(),
                                span: Span::dummy(),
                            },
                            span: Span::dummy(),
                        },
                        SelectClause {
                            expression_to_run: Expression::Call {
                                function: "c".to_string(),
                                arguments: vec![],
                                span: Span::dummy(),
                            },
                            result_variable: "z".to_string(),
                            expression_next: Expression::Variable {
                                name: "z".to_string(),
                                span: Span::dummy(),
                            },
                            span: Span::dummy(),
                        },
                    ],
                    span: Span::dummy(),
                }),
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = RedundantSelectAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_nested_single_branch_select() {
        let func = create_test_function(
            "test",
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: Span::dummy(),
                },
                body: vec![Statement::Assignment {
                    variable: "nested".to_string(),
                    expression: Expression::Select(SelectExpression {
                        clauses: vec![SelectClause {
                            expression_to_run: Expression::Call {
                                function: "single".to_string(),
                                arguments: vec![],
                                span: Span::dummy(),
                            },
                            result_variable: "x".to_string(),
                            expression_next: Expression::Variable {
                                name: "x".to_string(),
                                span: Span::dummy(),
                            },
                            span: Span::dummy(),
                        }],
                        span: Span::new(10, 30),
                    }),
                    span: Span::dummy(),
                }],
                span: Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = RedundantSelectAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
    }
}
