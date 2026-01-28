use super::*;
use crate::ast::{
    Definition, Expression, Function, FunctionBody, Module, Parameter, SelectClause,
    SelectExpression, Statement, Type as AstType,
};

fn create_test_module(definitions: Vec<Definition>) -> Module {
    Module {
        definitions,
        span: crate::types::Span::dummy(),
        file_id: 0,
    }
}

fn create_test_function(
    name: &str,
    parameters: Vec<Parameter>,
    return_type: AstType,
    statements: Vec<Statement>,
) -> Function {
    Function {
        name: name.to_string(),
        parameters,
        return_type,
        body: FunctionBody {
            statements,
            span: crate::types::Span::dummy(),
        },
        span: crate::types::Span::dummy(),
        documentation: None,
    }
}

fn create_parameter(name: &str, param_type: AstType) -> Parameter {
    Parameter {
        name: name.to_string(),
        param_type,
        span: crate::types::Span::dummy(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_function_with_string_parameter() {
        let func = create_test_function(
            "greet",
            vec![create_parameter("name", AstType::String)],
            AstType::String,
            vec![Statement::Return(Expression::Variable {
                name: "name".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module, 0).is_ok());
    }

    #[test]
    fn test_unknown_variable_error() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Unit,
            vec![Statement::Return(Expression::Variable {
                name: "unknown".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::UnknownVariable { .. }
        ));
    }

    #[test]
    fn test_function_call_with_correct_arguments() {
        let greet_func = create_test_function(
            "greet",
            vec![create_parameter("name", AstType::String)],
            AstType::String,
            vec![Statement::Return(Expression::Variable {
                name: "name".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "greet".to_string(),
                arguments: vec![Expression::StringLiteral {
                    value: "Alice".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(greet_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module, 0).is_ok());
    }

    #[test]
    fn test_function_call_with_wrong_argument_type() {
        let greet_func = create_test_function(
            "greet",
            vec![create_parameter("name", AstType::String)],
            AstType::String,
            vec![Statement::Return(Expression::Variable {
                name: "name".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "greet".to_string(),
                arguments: vec![Expression::BooleanLiteral {
                    value: true,
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(greet_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::ArgumentTypeMismatch { .. }
        ));
    }

    #[test]
    fn test_function_call_with_wrong_argument_count() {
        let greet_func = create_test_function(
            "greet",
            vec![create_parameter("name", AstType::String)],
            AstType::String,
            vec![Statement::Return(Expression::Variable {
                name: "name".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "greet".to_string(),
                arguments: vec![],
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(greet_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::ArgumentCountMismatch { .. }
        ));
    }

    #[test]
    fn test_placeholder_arguments_are_allowed() {
        let test_func = create_test_function(
            "test",
            vec![create_parameter("data", AstType::String)],
            AstType::Unit,
            vec![],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "test".to_string(),
                arguments: vec![Expression::Placeholder {
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(test_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        if let Err(ref e) = result {
            println!("Error: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_let_statement_type_inference() {
        let get_name_func = create_test_function(
            "get_name",
            vec![],
            AstType::String,
            vec![Statement::Return(Expression::StringLiteral {
                value: "Alice".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![
                Statement::Assignment {
                    variable: "name".to_string(),
                    expression: Expression::Call {
                        function: "get_name".to_string(),
                        arguments: vec![],
                        span: crate::types::Span::dummy(),
                    },
                    span: crate::types::Span::dummy(),
                },
                Statement::ExpressionStatement(Expression::Variable {
                    name: "name".to_string(),
                    span: crate::types::Span::dummy(),
                }),
            ],
        );

        let module = create_test_module(vec![
            Definition::Function(get_name_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        if let Err(ref e) = result {
            println!("Error: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_variable_assignment_type_mismatch() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Unit,
            vec![
                Statement::Assignment {
                    variable: "flag".to_string(),
                    expression: Expression::BooleanLiteral {
                        value: true,
                        span: crate::types::Span::dummy(),
                    },
                    span: crate::types::Span::dummy(),
                },
                Statement::VariableAssignment {
                    variable: "flag".to_string(),
                    expression: Expression::StringLiteral {
                        value: "hello".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                    span: crate::types::Span::dummy(),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::VariableTypeMismatch { .. }
        ));
    }

    #[test]
    fn test_if_condition_must_be_boolean() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Unit,
            vec![Statement::If {
                condition: Expression::StringLiteral {
                    value: "hello".to_string(),
                    span: crate::types::Span::dummy(),
                },
                body: vec![],
                else_body: None,
                span: crate::types::Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::TypeMismatch { .. }
        ));
    }

    #[test]
    fn test_while_condition_must_be_boolean() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Unit,
            vec![Statement::While {
                condition: Expression::StringLiteral {
                    value: "hello".to_string(),
                    span: crate::types::Span::dummy(),
                },
                body: vec![],
                span: crate::types::Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::TypeMismatch { .. }
        ));
    }

    #[test]
    fn test_return_type_mismatch() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::String,
            vec![Statement::Return(Expression::BooleanLiteral {
                value: true,
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::ReturnTypeMismatch { .. }
        ));
    }

    #[test]
    fn test_select_all_branches_same_type() {
        let add_func = create_test_function(
            "add",
            vec![
                create_parameter("a", AstType::String),
                create_parameter("b", AstType::String),
            ],
            AstType::String,
            vec![Statement::Return(Expression::StringLiteral {
                value: "result".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let concat_func = create_test_function(
            "concat",
            vec![
                create_parameter("value2", AstType::String),
                create_parameter("value1", AstType::String),
            ],
            AstType::String,
            vec![Statement::Return(Expression::StringLiteral {
                value: "concatenated".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::String,
            vec![Statement::Return(Expression::Select(SelectExpression {
                clauses: vec![
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "add".to_string(),
                            arguments: vec![
                                Expression::Placeholder {
                                    span: crate::types::Span::dummy(),
                                },
                                Expression::Placeholder {
                                    span: crate::types::Span::dummy(),
                                },
                            ],
                            span: crate::types::Span::dummy(),
                        },
                        result_variable: "sum".to_string(),
                        expression_next: Expression::Variable {
                            name: "sum".to_string(),
                            span: crate::types::Span::dummy(),
                        },
                        span: crate::types::Span::dummy(),
                    },
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "concat".to_string(),
                            arguments: vec![
                                Expression::Placeholder {
                                    span: crate::types::Span::dummy(),
                                },
                                Expression::Placeholder {
                                    span: crate::types::Span::dummy(),
                                },
                            ],
                            span: crate::types::Span::dummy(),
                        },
                        result_variable: "text".to_string(),
                        expression_next: Expression::Variable {
                            name: "text".to_string(),
                            span: crate::types::Span::dummy(),
                        },
                        span: crate::types::Span::dummy(),
                    },
                ],
                span: crate::types::Span::dummy(),
            }))],
        );

        let module = create_test_module(vec![
            Definition::Function(add_func),
            Definition::Function(concat_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module, 0).is_ok());
    }

    #[test]
    fn test_select_branch_type_mismatch() {
        let get_string_func = create_test_function(
            "get_string",
            vec![],
            AstType::String,
            vec![Statement::Return(Expression::StringLiteral {
                value: "text".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let get_bool_func = create_test_function(
            "get_bool",
            vec![],
            AstType::Boolean,
            vec![Statement::Return(Expression::BooleanLiteral {
                value: true,
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::String,
            vec![Statement::Return(Expression::Select(SelectExpression {
                clauses: vec![
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "get_string".to_string(),
                            arguments: vec![],
                            span: crate::types::Span::dummy(),
                        },
                        result_variable: "str_result".to_string(),
                        expression_next: Expression::Variable {
                            name: "str_result".to_string(),
                            span: crate::types::Span::dummy(),
                        },
                        span: crate::types::Span::dummy(),
                    },
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "get_bool".to_string(),
                            arguments: vec![],
                            span: crate::types::Span::dummy(),
                        },
                        result_variable: "bool_result".to_string(),
                        expression_next: Expression::Variable {
                            name: "bool_result".to_string(),
                            span: crate::types::Span::dummy(),
                        },
                        span: crate::types::Span::dummy(),
                    },
                ],
                span: crate::types::Span::dummy(),
            }))],
        );

        let module = create_test_module(vec![
            Definition::Function(get_string_func),
            Definition::Function(get_bool_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::SelectBranchTypeMismatch { .. }
        ));
    }

    #[test]
    fn test_external_function_type_checking() {
        use crate::ast::ExternalFunction;

        let ext_func = ExternalFunction {
            name: "concat".to_string(),
            parameters: vec![
                create_parameter("value1", AstType::String),
                create_parameter("id", AstType::String),
            ],
            return_type: AstType::String,
            span: crate::types::Span::dummy(),
        };

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "concat".to_string(),
                arguments: vec![
                    Expression::StringLiteral {
                        value: "hello".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                    Expression::StringLiteral {
                        value: "world".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                ],
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::ExternalFunction(ext_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module, 0).is_ok());
    }

    #[test]
    fn test_nested_scope_variable_isolation() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Unit,
            vec![
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: crate::types::Span::dummy(),
                    },
                    body: vec![Statement::Assignment {
                        variable: "inner_var".to_string(),
                        expression: Expression::StringLiteral {
                            value: "hello".to_string(),
                            span: crate::types::Span::dummy(),
                        },
                        span: crate::types::Span::dummy(),
                    }],
                    else_body: None,
                    span: crate::types::Span::dummy(),
                },
                Statement::ExpressionStatement(Expression::Variable {
                    name: "inner_var".to_string(),
                    span: crate::types::Span::dummy(),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::UnknownVariable { .. }
        ));
    }

    #[test]
    fn test_variable_shadowing_should_not_leak() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::String,
            vec![
                Statement::Assignment {
                    variable: "shared".to_string(),
                    expression: Expression::StringLiteral {
                        value: "foo".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                    span: crate::types::Span::dummy(),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral {
                        value: true,
                        span: crate::types::Span::dummy(),
                    },
                    body: vec![Statement::Assignment {
                        variable: "shared".to_string(),
                        expression: Expression::BooleanLiteral {
                            value: true,
                            span: crate::types::Span::dummy(),
                        },
                        span: crate::types::Span::dummy(),
                    }],
                    else_body: None,
                    span: crate::types::Span::dummy(),
                },
                // After if block, shared should still be String type from outer scope
                Statement::Return(Expression::Variable {
                    name: "shared".to_string(),
                    span: crate::types::Span::dummy(),
                }),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module, 0);
        // This should pass - the Boolean assignment in the if block should not affect outer scope
        if let Err(ref e) = result {
            println!("Error: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_deeply_nested_scoping() {
        // This demonstrates a case where proper scope chaining matters
        let func = create_test_function(
            "test",
            vec![],
            AstType::String,
            vec![Statement::If {
                condition: Expression::BooleanLiteral {
                    value: true,
                    span: crate::types::Span::dummy(),
                },
                body: vec![
                    Statement::Assignment {
                        variable: "x".to_string(),
                        expression: Expression::StringLiteral {
                            value: "outer".to_string(),
                            span: crate::types::Span::dummy(),
                        },
                        span: crate::types::Span::dummy(),
                    },
                    Statement::If {
                        condition: Expression::BooleanLiteral {
                            value: true,
                            span: crate::types::Span::dummy(),
                        },
                        body: vec![
                            Statement::Assignment {
                                variable: "y".to_string(),
                                expression: Expression::StringLiteral {
                                    value: "middle".to_string(),
                                    span: crate::types::Span::dummy(),
                                },
                                span: crate::types::Span::dummy(),
                            },
                            Statement::If {
                                condition: Expression::BooleanLiteral {
                                    value: true,
                                    span: crate::types::Span::dummy(),
                                },
                                body: vec![
                                    Statement::Assignment {
                                        variable: "z".to_string(),
                                        expression: Expression::StringLiteral {
                                            value: "inner".to_string(),
                                            span: crate::types::Span::dummy(),
                                        },
                                        span: crate::types::Span::dummy(),
                                    },
                                    Statement::ExpressionStatement(Expression::Variable {
                                        name: "x".to_string(),
                                        span: crate::types::Span::dummy(),
                                    }),
                                    Statement::ExpressionStatement(Expression::Variable {
                                        name: "y".to_string(),
                                        span: crate::types::Span::dummy(),
                                    }),
                                ],
                                else_body: None,
                                span: crate::types::Span::dummy(),
                            },
                            // z should not be accessible here
                        ],
                        else_body: None,
                        span: crate::types::Span::dummy(),
                    },
                    Statement::Return(Expression::Variable {
                        name: "x".to_string(),
                        span: crate::types::Span::dummy(),
                    }),
                ],
                else_body: None,
                span: crate::types::Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        // This should pass with proper scope chaining
        let result = checker.check_module(&module, 0);
        if let Err(ref e) = result {
            println!("Error: {}", e);
        }
        assert!(result.is_ok());
    }
}
