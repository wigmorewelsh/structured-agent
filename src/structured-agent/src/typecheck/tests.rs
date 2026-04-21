use super::*;
use crate::ast::{
    AstTrait, Definition, Expression, Function, FunctionBody, Module, Parameter, SelectClause,
    SelectExpression, Statement, Type as AstType,
};
use crate::compiler::parser::parse_program;
use combine::{Parser, stream::position::IndexPositioner};

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
        type_params: vec![],
        parameters,
        return_type,
        body: FunctionBody {
            statements,
            span: crate::types::Span::dummy(),
        },
        span: crate::types::Span::dummy(),
        documentation: None,
        is_pub: false,
    }
}

fn create_parameter(name: &str, param_type: AstType) -> Parameter {
    Parameter {
        name: name.to_string(),
        param_type,
        span: crate::types::Span::dummy(),
    }
}

fn create_generic_test_function(
    name: &str,
    type_params: Vec<crate::ast::TypeParam>,
    parameters: Vec<Parameter>,
    return_type: AstType,
    statements: Vec<Statement>,
) -> Function {
    Function {
        name: name.to_string(),
        type_params,
        parameters,
        return_type,
        body: FunctionBody {
            statements,
            span: crate::types::Span::dummy(),
        },
        span: crate::types::Span::dummy(),
        documentation: None,
        is_pub: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nonempty::NonEmpty;
    use std::sync::Arc;

    fn check(module: crate::ast::Module) -> Result<(), Vec<crate::typecheck::TypeError>> {
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: true,
            file_id: 0,
            is_inline: false,
        };
        TypeChecker::new()
            .check(&[parsed], &std::collections::HashMap::new())
            .map(|_| ())
    }

    #[test]
    fn test_valid_function_with_string_parameter() {
        let func = create_test_function(
            "greet",
            vec![create_parameter("name", AstType::simple("String"))],
            AstType::simple("String"),
            vec![Statement::Return(Expression::Variable {
                name: "name".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        assert!(check(module).is_ok());
    }

    #[test]
    fn test_unknown_variable_error() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::Return(Expression::Variable {
                name: "unknown".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::UnknownVariable { .. }));
    }

    #[test]
    fn test_function_call_with_correct_arguments() {
        let greet_func = create_test_function(
            "greet",
            vec![create_parameter("name", AstType::simple("String"))],
            AstType::simple("String"),
            vec![Statement::Return(Expression::Variable {
                name: "name".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::simple("Unit"),
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
            Definition::Function(Arc::new(greet_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        assert!(check(module).is_ok());
    }

    #[test]
    fn test_function_call_with_wrong_argument_type() {
        let greet_func = create_test_function(
            "greet",
            vec![create_parameter("name", AstType::simple("String"))],
            AstType::simple("String"),
            vec![Statement::Return(Expression::Variable {
                name: "name".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::simple("Unit"),
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
            Definition::Function(Arc::new(greet_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(
            errors[0],
            TypeError::TypeMismatch { .. } | TypeError::ArgumentTypeMismatch { .. }
        ));
    }

    #[test]
    fn test_function_call_with_wrong_argument_count() {
        let greet_func = create_test_function(
            "greet",
            vec![create_parameter("name", AstType::simple("String"))],
            AstType::simple("String"),
            vec![Statement::Return(Expression::Variable {
                name: "name".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "greet".to_string(),
                arguments: vec![],
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(Arc::new(greet_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::ArgumentCountMismatch { .. }));
    }

    #[test]
    fn test_placeholder_arguments_are_allowed() {
        let test_func = create_test_function(
            "test",
            vec![create_parameter("data", AstType::simple("String"))],
            AstType::simple("Unit"),
            vec![],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "test".to_string(),
                arguments: vec![Expression::Placeholder {
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(Arc::new(test_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
        if let Err(ref e) = result {
            println!("Error: {:?}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_let_statement_type_inference() {
        let get_name_func = create_test_function(
            "get_name",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringLiteral {
                value: "Alice".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::simple("Unit"),
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
            Definition::Function(Arc::new(get_name_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
        if let Err(ref e) = result {
            println!("Error: {:?}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_variable_assignment_type_mismatch() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("Unit"),
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::VariableTypeMismatch { .. }));
    }

    #[test]
    fn test_if_condition_must_be_boolean() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("Unit"),
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::TypeMismatch { .. }));
    }

    #[test]
    fn test_while_condition_must_be_boolean() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::While {
                condition: Expression::StringLiteral {
                    value: "hello".to_string(),
                    span: crate::types::Span::dummy(),
                },
                body: vec![],
                span: crate::types::Span::dummy(),
            }],
        );

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::TypeMismatch { .. }));
    }

    #[test]
    fn test_return_type_mismatch() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::BooleanLiteral {
                value: true,
                span: crate::types::Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::ReturnTypeMismatch { .. }));
    }

    #[test]
    fn test_select_all_branches_same_type() {
        let add_func = create_test_function(
            "add",
            vec![
                create_parameter("a", AstType::simple("String")),
                create_parameter("b", AstType::simple("String")),
            ],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringLiteral {
                value: "result".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let concat_func = create_test_function(
            "concat",
            vec![
                create_parameter("value2", AstType::simple("String")),
                create_parameter("value1", AstType::simple("String")),
            ],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringLiteral {
                value: "concatenated".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::simple("String"),
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
            Definition::Function(Arc::new(add_func)),
            Definition::Function(Arc::new(concat_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        assert!(check(module).is_ok());
    }

    #[test]
    fn test_select_branch_type_mismatch() {
        let get_string_func = create_test_function(
            "get_string",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringLiteral {
                value: "text".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );

        let get_bool_func = create_test_function(
            "get_bool",
            vec![],
            AstType::simple("Boolean"),
            vec![Statement::Return(Expression::BooleanLiteral {
                value: true,
                span: crate::types::Span::dummy(),
            })],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::simple("String"),
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
            Definition::Function(Arc::new(get_string_func)),
            Definition::Function(Arc::new(get_bool_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::TypeMismatch { .. }));
    }

    #[test]
    fn test_external_function_type_checking() {
        use crate::ast::ExternalFunction;

        let ext_func = ExternalFunction {
            name: "concat".to_string(),
            type_params: vec![],
            parameters: vec![
                create_parameter("value1", AstType::simple("String")),
                create_parameter("id", AstType::simple("String")),
            ],
            return_type: AstType::simple("String"),
            is_pub: false,
            span: crate::types::Span::dummy(),
        };

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::simple("Unit"),
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
            Definition::ExternalFunction(Arc::new(ext_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        assert!(check(module).is_ok());
    }

    #[test]
    fn test_nested_scope_variable_isolation() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("Unit"),
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::UnknownVariable { .. }));
    }

    #[test]
    fn test_variable_shadowing_should_not_leak() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("String"),
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        // This should pass - the Boolean assignment in the if block should not affect outer scope
        if let Err(ref e) = result {
            println!("Error: {:?}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_deeply_nested_scoping() {
        // This demonstrates a case where proper scope chaining matters
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("String"),
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        if let Err(ref e) = result {
            println!("Error: {:?}", e);
        }
        assert!(result.is_ok());
    }

    fn create_struct_definition(name: &str, fields: Vec<(&str, AstType)>) -> Definition {
        use crate::ast::{StructDefinition, StructField};
        Definition::Struct(Arc::new(StructDefinition {
            name: name.to_string(),
            type_params: vec![],
            fields: fields
                .into_iter()
                .map(|(field_name, field_type)| StructField {
                    name: field_name.to_string(),
                    field_type,
                    span: crate::types::Span::dummy(),
                })
                .collect(),
            span: crate::types::Span::dummy(),
        }))
    }

    #[test]
    fn test_struct_definition_registers_type() {
        let module = create_test_module(vec![
            create_struct_definition(
                "Point",
                vec![("x", AstType::simple("Int")), ("y", AstType::simple("Int"))],
            ),
            Definition::Function(Arc::new(create_test_function(
                "main",
                vec![],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_struct_type_in_function_parameter_is_valid() {
        let module = create_test_module(vec![
            create_struct_definition("Task", vec![("title", AstType::simple("String"))]),
            Definition::Function(Arc::new(create_test_function(
                "get_title",
                vec![create_parameter("t", AstType::simple("Task"))],
                AstType::simple("String"),
                vec![Statement::Return(Expression::FieldAccess {
                    base: Box::new(Expression::Variable {
                        name: "t".to_string(),
                        span: crate::types::Span::dummy(),
                    }),
                    field: "title".to_string(),
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_unknown_struct_type_in_parameter_is_error() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "foo",
                vec![create_parameter("x", AstType::simple("Unknown"))],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::UndefinedType { .. }));
    }

    #[test]
    fn test_struct_literal_valid() {
        let module = create_test_module(vec![
            create_struct_definition(
                "Point",
                vec![("x", AstType::simple("Int")), ("y", AstType::simple("Int"))],
            ),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::simple("Point"),
                vec![Statement::Return(Expression::StructLiteral {
                    struct_name: "Point".to_string(),
                    fields: vec![
                        (
                            "x".to_string(),
                            Expression::IntLiteral {
                                value: 1,
                                span: crate::types::Span::dummy(),
                            },
                        ),
                        (
                            "y".to_string(),
                            Expression::IntLiteral {
                                value: 2,
                                span: crate::types::Span::dummy(),
                            },
                        ),
                    ],
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_struct_literal_unknown_struct_is_error() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::StructLiteral {
                    struct_name: "Ghost".to_string(),
                    fields: vec![],
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::UnsupportedType { .. }));
    }

    #[test]
    fn test_struct_literal_unknown_field_is_error() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::simple("Int"))]),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::StructLiteral {
                    struct_name: "Point".to_string(),
                    fields: vec![(
                        "z".to_string(),
                        Expression::IntLiteral {
                            value: 1,
                            span: crate::types::Span::dummy(),
                        },
                    )],
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::UnknownField { .. }));
    }

    #[test]
    fn test_struct_literal_field_type_mismatch_is_error() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::simple("Int"))]),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::StructLiteral {
                    struct_name: "Point".to_string(),
                    fields: vec![(
                        "x".to_string(),
                        Expression::StringLiteral {
                            value: "wrong".to_string(),
                            span: crate::types::Span::dummy(),
                        },
                    )],
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(
            errors[0],
            TypeError::StructFieldTypeMismatch { .. }
        ));
    }

    #[test]
    fn test_struct_literal_missing_field_span_does_not_bleed() {
        let source = "struct Point {\n    x: Int,\n    y: Int,\n}\n\nfn main(): Int {\n    let p = Point { x: 1 }\n    return p.x\n}\n";
        let stream =
            combine::stream::position::Stream::with_positioner(source, IndexPositioner::default());
        let (module, _) = parse_program(0).parse(stream).unwrap();
        let error = check(module).unwrap_err().into_iter().next().unwrap();
        let TypeError::MissingField { span, .. } = error else {
            panic!("Expected MissingField, got {:?}", error);
        };
        let literal = "Point { x: 1 }";
        let literal_start = source.find(literal).unwrap();
        let literal_end = literal_start + literal.len();
        assert!(
            span.start >= literal_start,
            "span starts before the struct literal"
        );
        assert!(
            span.end <= literal_end,
            "MissingField span.end ({}) bleeds past closing brace of struct literal ({})",
            span.end,
            literal_end
        );
    }

    #[test]
    fn test_struct_literal_missing_field_is_error() {
        let module = create_test_module(vec![
            create_struct_definition(
                "Point",
                vec![("x", AstType::simple("Int")), ("y", AstType::simple("Int"))],
            ),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::simple("Point"),
                vec![Statement::Return(Expression::StructLiteral {
                    struct_name: "Point".to_string(),
                    fields: vec![(
                        "x".to_string(),
                        Expression::IntLiteral {
                            value: 1,
                            span: crate::types::Span::dummy(),
                        },
                    )],
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        let result = check(module);
        assert!(result.is_err());
        let error = result.unwrap_err().into_iter().next().unwrap();
        assert!(matches!(error, TypeError::MissingField { field_name, .. } if field_name == "y"));
    }

    #[test]
    fn test_field_access_valid() {
        let module = create_test_module(vec![
            create_struct_definition(
                "Point",
                vec![("x", AstType::simple("Int")), ("y", AstType::simple("Int"))],
            ),
            Definition::Function(Arc::new(create_test_function(
                "get_x",
                vec![create_parameter("p", AstType::simple("Point"))],
                AstType::simple("Int"),
                vec![Statement::Return(Expression::FieldAccess {
                    base: Box::new(Expression::Variable {
                        name: "p".to_string(),
                        span: crate::types::Span::dummy(),
                    }),
                    field: "x".to_string(),
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_field_access_unknown_field_is_error() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::simple("Int"))]),
            Definition::Function(Arc::new(create_test_function(
                "get_z",
                vec![create_parameter("p", AstType::simple("Point"))],
                AstType::simple("Int"),
                vec![Statement::Return(Expression::FieldAccess {
                    base: Box::new(Expression::Variable {
                        name: "p".to_string(),
                        span: crate::types::Span::dummy(),
                    }),
                    field: "z".to_string(),
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::UnknownField { .. }));
    }

    #[test]
    fn test_field_access_on_non_struct_is_error() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "bad",
                vec![create_parameter("s", AstType::simple("String"))],
                AstType::simple("Int"),
                vec![Statement::Return(Expression::FieldAccess {
                    base: Box::new(Expression::Variable {
                        name: "s".to_string(),
                        span: crate::types::Span::dummy(),
                    }),
                    field: "x".to_string(),
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::UnsupportedType { .. }));
    }

    #[test]
    fn test_struct_defined_after_function_that_uses_it_is_valid() {
        let module = create_test_module(vec![
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::simple("Point"),
                vec![Statement::Return(Expression::StructLiteral {
                    struct_name: "Point".to_string(),
                    fields: vec![(
                        "x".to_string(),
                        Expression::IntLiteral {
                            value: 0,
                            span: crate::types::Span::dummy(),
                        },
                    )],
                    span: crate::types::Span::dummy(),
                })],
            ))),
            create_struct_definition("Point", vec![("x", AstType::simple("Int"))]),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_struct_literal_as_call_argument_is_valid() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::simple("Int"))]),
            Definition::Function(Arc::new(create_test_function(
                "consume",
                vec![create_parameter("p", AstType::simple("Point"))],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                })],
            ))),
            Definition::Function(Arc::new(create_test_function(
                "make_and_pass",
                vec![],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::Call {
                    function: "consume".to_string(),
                    arguments: vec![Expression::StructLiteral {
                        struct_name: "Point".to_string(),
                        fields: vec![(
                            "x".to_string(),
                            Expression::IntLiteral {
                                value: 1,
                                span: crate::types::Span::dummy(),
                            },
                        )],
                        span: crate::types::Span::dummy(),
                    }],
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_extern_fn_with_struct_return_type_is_valid() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::simple("Int"))]),
            Definition::ExternalFunction(Arc::new(crate::ast::ExternalFunction {
                name: "get_point".to_string(),
                type_params: vec![],
                parameters: vec![],
                return_type: AstType::simple("Point"),
                is_pub: false,
                span: crate::types::Span::dummy(),
            })),
            Definition::Function(Arc::new(create_test_function(
                "main",
                vec![],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_struct_literal_duplicate_field_is_error() {
        let module = create_test_module(vec![
            create_struct_definition(
                "Point",
                vec![("x", AstType::simple("Int")), ("y", AstType::simple("Int"))],
            ),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::simple("Point"),
                vec![Statement::Return(Expression::StructLiteral {
                    struct_name: "Point".to_string(),
                    fields: vec![
                        (
                            "x".to_string(),
                            Expression::IntLiteral {
                                value: 1,
                                span: crate::types::Span::dummy(),
                            },
                        ),
                        (
                            "x".to_string(),
                            Expression::IntLiteral {
                                value: 2,
                                span: crate::types::Span::dummy(),
                            },
                        ),
                        (
                            "y".to_string(),
                            Expression::IntLiteral {
                                value: 3,
                                span: crate::types::Span::dummy(),
                            },
                        ),
                    ],
                    span: crate::types::Span::dummy(),
                })],
            ))),
        ]);
        let result = check(module);
        assert!(result.is_err());
        let error = result.unwrap_err().into_iter().next().unwrap();
        assert!(matches!(error, TypeError::DuplicateField { field_name, .. } if field_name == "x"));
    }

    #[test]
    fn test_extern_fn_with_unknown_struct_return_type_is_error() {
        let module = create_test_module(vec![Definition::ExternalFunction(Arc::new(
            crate::ast::ExternalFunction {
                name: "get_ghost".to_string(),
                type_params: vec![],
                parameters: vec![],
                return_type: AstType::simple("Ghost"),
                is_pub: false,
                span: crate::types::Span::dummy(),
            },
        ))]);
        let result = check(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(errors[0], TypeError::UndefinedType { .. }));
    }

    #[test]
    fn test_struct_typed_function_typechecks_via_parser() {
        let input = "struct MyStruct {\n    value: String,\n}\nfn foo(x: MyStruct): MyStruct {\n    return x\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let result = check(module);
        assert!(
            result.is_ok(),
            "struct-typed function should type-check: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_generic_type_param_single_parsed_correctly() {
        let input = "fn identity<T>(x: T): T {\n    return x\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        if let crate::ast::Definition::Function(f) = &module.definitions[0] {
            assert_eq!(f.type_params, vec!["T"]);
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_generic_type_param_multiple_parsed_correctly() {
        let input = "fn pair<T, U>(a: T, b: U): T {\n    return a\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        if let crate::ast::Definition::Function(f) = &module.definitions[0] {
            assert_eq!(f.type_params, vec!["T", "U"]);
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_generic_call_wrong_type_produces_mismatch() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType {
                    name: "List".to_string(),
                    args: vec![AstType::simple("T")],
                },
            )],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("T")],
            },
            vec![],
        );
        let caller = create_test_function(
            "caller",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("String")],
            },
            vec![Statement::Return(Expression::Call {
                function: "head".to_string(),
                arguments: vec![Expression::Variable {
                    name: "s".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = create_test_module(vec![
            Definition::Function(Arc::new(head)),
            Definition::Function(Arc::new(caller)),
        ]);
        let result = check(module);
        let errors = result.unwrap_err();
        assert!(
            matches!(&errors[0], TypeError::ArgumentTypeMismatch { expected, found, .. }
                if expected == "List<T>" && found == "String"),
            "expected ArgumentTypeMismatch with concrete type names, got {:?}",
            errors
        );
    }

    #[test]
    fn test_generic_extern_fn_with_type_params_type_checks() {
        let ext_func = crate::ast::ExternalFunction {
            name: "wrap".to_string(),
            type_params: vec!["T".into()],
            parameters: vec![create_parameter("value", AstType::simple("T"))],
            return_type: AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("T")],
            },
            is_pub: false,
            span: crate::types::Span::dummy(),
        };
        let caller = create_test_function(
            "main",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("String")],
            },
            vec![Statement::Return(Expression::Call {
                function: "wrap".to_string(),
                arguments: vec![Expression::Variable {
                    name: "s".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = create_test_module(vec![
            Definition::ExternalFunction(Arc::new(ext_func)),
            Definition::Function(Arc::new(caller)),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_generic_fn_type_var_only_in_return_type_call_succeeds_with_unresolved_generic() {
        let make_none = create_generic_test_function(
            "make_none",
            vec!["T".into()],
            vec![],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("T")],
            },
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "make_none".to_string(),
                arguments: vec![],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = create_test_module(vec![
            Definition::Function(Arc::new(make_none)),
            Definition::Function(Arc::new(caller)),
        ]);
        assert!(
            check(module).is_ok(),
            "call to fn with T only in return type succeeds; T stays unresolved as Generic(\"T\")"
        );
    }

    #[test]
    fn test_generic_call_result_used_in_type_sensitive_context() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType {
                    name: "List".to_string(),
                    args: vec![AstType::simple("T")],
                },
            )],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("T")],
            },
            vec![],
        );
        let consume = create_test_function(
            "consume",
            vec![create_parameter(
                "item",
                AstType {
                    name: "Option".to_string(),
                    args: vec![AstType::simple("String")],
                },
            )],
            AstType::simple("Unit"),
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![create_parameter(
                "xs",
                AstType {
                    name: "List".to_string(),
                    args: vec![AstType::simple("String")],
                },
            )],
            AstType::simple("Unit"),
            vec![
                Statement::Assignment {
                    variable: "result".to_string(),
                    expression: Expression::Call {
                        function: "head".to_string(),
                        arguments: vec![Expression::Variable {
                            name: "xs".to_string(),
                            span: crate::types::Span::dummy(),
                        }],
                        span: crate::types::Span::dummy(),
                    },
                    span: crate::types::Span::dummy(),
                },
                Statement::ExpressionStatement(Expression::Call {
                    function: "consume".to_string(),
                    arguments: vec![Expression::Variable {
                        name: "result".to_string(),
                        span: crate::types::Span::dummy(),
                    }],
                    span: crate::types::Span::dummy(),
                }),
            ],
        );
        let module = create_test_module(vec![
            Definition::Function(Arc::new(head)),
            Definition::Function(Arc::new(consume)),
            Definition::Function(Arc::new(caller)),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_unknown_type_variable_in_non_generic_function_is_error() {
        let bad = create_generic_test_function(
            "bad",
            vec![],
            vec![create_parameter("x", AstType::simple("T"))],
            AstType::simple("T"),
            vec![],
        );
        let module = create_test_module(vec![Definition::Function(Arc::new(bad))]);
        let result = check(module);
        let errors = result.unwrap_err();
        assert!(
            matches!(&errors[0], TypeError::UndefinedType { name, .. } if name == "T"),
            "expected UndefinedType for unknown type variable, got {:?}",
            errors
        );
    }

    #[test]
    fn test_bound_generic_in_function_signature_is_valid() {
        let func = create_generic_test_function(
            "identity",
            vec![crate::ast::TypeParam {
                name: "T".to_string(),
                bounds: vec![],
            }],
            vec![create_parameter("x", AstType::simple("T"))],
            AstType::simple("T"),
            vec![],
        );
        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_unbound_type_parameter_in_function_signature_is_error() {
        let func = create_generic_test_function(
            "bad",
            vec![],
            vec![create_parameter("x", AstType::simple("T"))],
            AstType::simple("Unit"),
            vec![],
        );
        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);
        let result = check(module);
        let errors = result.unwrap_err();
        assert!(
            matches!(&errors[0], TypeError::UndefinedType { name, .. } if name == "T"),
            "expected UndefinedType, got {:?}",
            errors
        );
    }

    #[test]
    fn test_generic_in_struct_field_is_error() {
        let module = create_test_module(vec![create_struct_definition(
            "Wrapper",
            vec![("value", AstType::simple("T"))],
        )]);
        let result = check(module);
        let errors = result.unwrap_err();
        assert!(
            matches!(&errors[0], TypeError::UndefinedType { name, .. } if name == "T"),
            "expected UndefinedType for generic struct field, got {:?}",
            errors
        );
    }

    #[test]
    fn test_user_defined_generic_struct_as_function_parameter_is_valid() {
        use crate::ast::{StructDefinition, StructField};
        let pair_struct = Definition::Struct(Arc::new(StructDefinition {
            name: "Pair".to_string(),
            type_params: vec![crate::ast::TypeParam {
                name: "T".to_string(),
                bounds: vec![],
            }],
            fields: vec![
                StructField {
                    name: "first".to_string(),
                    field_type: AstType::simple("T"),
                    span: crate::types::Span::dummy(),
                },
                StructField {
                    name: "second".to_string(),
                    field_type: AstType::simple("T"),
                    span: crate::types::Span::dummy(),
                },
            ],
            span: crate::types::Span::dummy(),
        }));
        let pair_string = AstType {
            name: "Pair".to_string(),
            args: vec![AstType::simple("String")],
        };
        let func = create_test_function(
            "identity",
            vec![create_parameter("x", pair_string.clone())],
            pair_string,
            vec![Statement::Return(Expression::Variable {
                name: "x".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let module = create_test_module(vec![pair_struct, Definition::Function(Arc::new(func))]);
        let result = check(module);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    #[ignore = "impl/trait refactor in progress"]
    fn test_trait_bound_satisfied_for_int() {
        let input = "trait Add {\n    fn add(self: Self, other: Self): Self\n}\nimpl Int: Add {\n    fn add(self: Int, other: Int): Int {\n        return self\n    }\n}\nfn double<T: Add>(x: T): T {\n    return x\n}\nfn main(): Int {\n    return double(42)\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let result = check(module);
        assert!(
            result.is_ok(),
            "Int satisfies Add, should type check: {:?}",
            result
        );
    }

    #[test]
    #[ignore = "impl/trait refactor in progress"]
    fn test_trait_bound_not_satisfied_for_string() {
        let input = "trait Add {\n    fn add(self: Self, other: Self): Self\n}\nfn double<T: Add>(x: T): T {\n    return x\n}\nfn main(): String {\n    return double(\"hello\")\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let result = check(module);
        let errors = result.unwrap_err();
        assert!(
            matches!(&errors[0], TypeError::TraitBoundNotSatisfied { type_name, trait_name, .. }
                if type_name == "String" && trait_name == "Add"),
            "String does not satisfy Add, expected TraitBoundNotSatisfied, got {:?}",
            errors
        );
    }

    #[test]
    #[ignore = "impl/trait refactor in progress"]
    fn test_trait_declaration_and_impl_valid() {
        let input = "struct Vec2 {\n    x: Int,\n    y: Int,\n}\ntrait Add {\n    fn add(self: Self, other: Self): Self\n}\nimpl Vec2: Add {\n    fn add(self: Vec2, other: Vec2): Vec2 {\n        return self\n    }\n}\nfn combine<T: Add>(a: T, b: T): T {\n    return a\n}\nfn main(): Vec2 {\n    let v = Vec2 { x: 1, y: 2 }\n    return combine(v, v)\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let result = check(module);
        assert!(
            result.is_ok(),
            "Vec2 implements Add, should type check: {:?}",
            result
        );
    }

    #[test]
    #[ignore]
    fn test_trait_impl_missing_function_is_error() {
        let input = "struct Foo {\n    x: Int,\n}\ntrait Add {\n    fn add(self: Self, other: Self): Self\n}\nimpl Foo: Add {\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let errors = check(module).unwrap_err();
        assert!(
            matches!(&errors[0], TypeError::TraitImplMissingFunction { function_name, .. }
                if function_name == "add"),
            "impl missing 'add' should be error, got {:?}",
            errors
        );
    }

    #[test]
    #[ignore]
    fn test_unknown_trait_in_impl_is_error() {
        let input = "struct Foo {\n    x: Int,\n}\nimpl Foo: NonExistent {\n    fn something(self: Foo): Foo { return self }\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let errors = check(module).unwrap_err();
        assert!(
            matches!(&errors[0], TypeError::UnknownTrait { name, .. } if name == "NonExistent"),
            "impl for unknown trait should error, got {:?}",
            errors
        );
    }
}

#[cfg(test)]
mod typed_ast_tests {
    use super::*;
    use crate::typecheck::{FunctionKind, TypedCheckerAstRef};
    use crate::typed_ast;
    use nonempty::NonEmpty;

    use std::sync::Arc;
    use structured_agent_runtime::Type as RT;
    use structured_agent_runtime::symbols::{FunctionName, ImplKey, ModuleName};

    fn check_typed(module: &Module) -> typed_ast::Module {
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("main".to_string()),
            module: module.clone(),
            is_entry: false,
            file_id: 0,
            is_inline: false,
        };
        let typed_metadata = TypeChecker::new()
            .check(&[parsed], &std::collections::HashMap::new())
            .unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module().to_string() != "main" {
                    return None;
                }
                if let TypedCheckerAstRef::Function(func, _) = &f.ast_ref {
                    Some(typed_ast::Definition::Function((**func).clone()))
                } else {
                    None
                }
            })
            .collect();
        typed_ast::Module {
            definitions,
            span: crate::types::Span::dummy(),
            file_id: 0,
        }
    }

    fn first_function(module: &typed_ast::Module) -> &typed_ast::Function {
        module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    Some(f)
                } else {
                    None
                }
            })
            .unwrap()
    }

    fn stmt_expr(stmt: &typed_ast::Statement) -> &typed_ast::Expression {
        match stmt {
            typed_ast::Statement::ExpressionStatement(e) => e,
            typed_ast::Statement::Return(e) => e,
            typed_ast::Statement::Injection(e) => e,
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn string_literal_has_string_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringLiteral {
                value: "hello".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::string());
    }

    #[test]
    fn boolean_literal_has_boolean_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("Boolean"),
            vec![Statement::Return(Expression::BooleanLiteral {
                value: true,
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::boolean());
    }

    #[test]
    fn int_literal_has_int_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("Int"),
            vec![Statement::Return(Expression::IntLiteral {
                value: 42,
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::int());
    }

    #[test]
    fn unit_literal_has_unit_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::Return(Expression::UnitLiteral {
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::unit());
    }

    #[test]
    fn call_carries_solved_type_arguments() {
        let code = r#"
            mod test
            fn test_func<T: Int>(x: T): T { return x }
            fn main(): () { test_func(42) }
        "#;
        let stream = combine::stream::position::Stream::with_positioner(
            code,
            combine::stream::position::IndexPositioner::default(),
        );
        let (module, _) = crate::compiler::parser::parse_program(0)
            .parse(stream)
            .unwrap();
        let parsed = crate::ast::ParsedModule {
            name: nonempty::NonEmpty::new("test".to_string()),
            is_entry: false,
            is_inline: false,
            file_id: 0,
            module,
        };
        let mut checker = super::TypeChecker::new();
        let metadata = checker
            .check(&[parsed], &std::collections::HashMap::new())
            .unwrap();

        let main_fn = metadata
            .functions
            .values()
            .find(|f| f.name.name() == "main")
            .unwrap();
        let crate::typecheck::refs::TypedCheckerAstRef::Function(f, _) = &main_fn.ast_ref else {
            panic!()
        };

        let stmt = f.body.statements.first().unwrap();
        let crate::typed_ast::Statement::ExpressionStatement(crate::typed_ast::Expression::Call {
            type_arguments,
            ..
        }) = stmt
        else {
            panic!()
        };

        assert_eq!(type_arguments.len(), 1);
    }

    #[test]
    fn variable_expression_carries_declared_type() {
        let func = create_test_function(
            "f",
            vec![create_parameter("x", AstType::simple("Int"))],
            AstType::simple("Int"),
            vec![Statement::Return(Expression::Variable {
                name: "x".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::int());
        assert!(matches!(expr, typed_ast::Expression::Variable { name, .. } if name == "x"));
    }

    #[test]
    fn call_carries_return_type_and_resolved_name() {
        let callee = create_test_function(
            "get_value",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringLiteral {
                value: "v".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let caller = create_test_function(
            "f",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::Call {
                function: "get_value".to_string(),
                arguments: vec![],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(callee)),
            Definition::Function(Arc::new(caller)),
        ]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "f" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = stmt_expr(f.body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::string());
        assert!(matches!(
            expr,
            typed_ast::Expression::Call { resolved, kind: FunctionKind::Bytecode, .. }
            if resolved == &FunctionName::new(ModuleName::new(NonEmpty::new("main".to_string())), "get_value")
        ));
    }

    #[test]
    fn placeholder_in_call_carries_parameter_type() {
        let callee = create_test_function(
            "process",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType::simple("Unit"),
            vec![Statement::Return(Expression::UnitLiteral {
                span: crate::types::Span::dummy(),
            })],
        );
        let caller = create_test_function(
            "f",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::Return(Expression::Call {
                function: "process".to_string(),
                arguments: vec![Expression::Placeholder {
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(callee)),
            Definition::Function(Arc::new(caller)),
        ]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "f" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = stmt_expr(f.body.statements.first().unwrap());
        if let typed_ast::Expression::Call { arguments, .. } = expr {
            assert_eq!(arguments[0].ty(), &RT::string());
            assert!(matches!(
                arguments[0],
                typed_ast::Expression::Placeholder { .. }
            ));
        } else {
            panic!("expected Call");
        }
    }

    #[test]
    fn assignment_statement_wraps_typed_expression() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("Unit"),
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::IntLiteral {
                        value: 1,
                        span: crate::types::Span::dummy(),
                    },
                    span: crate::types::Span::dummy(),
                },
                Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                }),
            ],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let stmt = first_function(&module).body.statements.first().unwrap();
        if let typed_ast::Statement::Assignment { expression, .. } = stmt {
            assert_eq!(expression.ty(), &RT::int());
        } else {
            panic!("expected Assignment");
        }
    }

    #[test]
    fn list_literal_has_list_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType {
                name: "List".to_string(),
                args: vec![AstType::simple("Int")],
            },
            vec![Statement::Return(Expression::ListLiteral {
                elements: vec![
                    Expression::IntLiteral {
                        value: 1,
                        span: crate::types::Span::dummy(),
                    },
                    Expression::IntLiteral {
                        value: 2,
                        span: crate::types::Span::dummy(),
                    },
                ],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::list(RT::int()));
    }

    #[test]
    fn if_else_expression_has_branch_type() {
        let func = create_test_function(
            "f",
            vec![create_parameter("flag", AstType::simple("Boolean"))],
            AstType::simple("Int"),
            vec![Statement::Return(Expression::IfElse {
                condition: Box::new(Expression::Variable {
                    name: "flag".to_string(),
                    span: crate::types::Span::dummy(),
                }),
                then_expr: Box::new(Expression::IntLiteral {
                    value: 1,
                    span: crate::types::Span::dummy(),
                }),
                else_expr: Box::new(Expression::IntLiteral {
                    value: 2,
                    span: crate::types::Span::dummy(),
                }),
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::int());
        assert!(matches!(expr, typed_ast::Expression::IfElse { .. }));
    }

    #[test]
    fn if_statement_wraps_typed_condition_and_body() {
        let func = create_test_function(
            "f",
            vec![create_parameter("flag", AstType::simple("Boolean"))],
            AstType::simple("Unit"),
            vec![
                Statement::If {
                    condition: Expression::Variable {
                        name: "flag".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                    body: vec![Statement::Return(Expression::UnitLiteral {
                        span: crate::types::Span::dummy(),
                    })],
                    else_body: None,
                    span: crate::types::Span::dummy(),
                },
                Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                }),
            ],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let stmt = first_function(&module).body.statements.first().unwrap();
        if let typed_ast::Statement::If {
            condition, body, ..
        } = stmt
        {
            assert_eq!(condition.ty(), &RT::boolean());
            assert!(!body.is_empty());
        } else {
            panic!("expected If");
        }
    }

    #[test]
    fn struct_literal_has_struct_type() {
        let struct_def = crate::ast::StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            fields: vec![
                crate::ast::StructField {
                    name: "x".to_string(),
                    field_type: AstType::simple("Int"),
                    span: crate::types::Span::dummy(),
                },
                crate::ast::StructField {
                    name: "y".to_string(),
                    field_type: AstType::simple("Int"),
                    span: crate::types::Span::dummy(),
                },
            ],
            span: crate::types::Span::dummy(),
        };
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("Point"),
            vec![Statement::Return(Expression::StructLiteral {
                struct_name: "Point".to_string(),
                fields: vec![
                    (
                        "x".to_string(),
                        Expression::IntLiteral {
                            value: 1,
                            span: crate::types::Span::dummy(),
                        },
                    ),
                    (
                        "y".to_string(),
                        Expression::IntLiteral {
                            value: 2,
                            span: crate::types::Span::dummy(),
                        },
                    ),
                ],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Struct(Arc::new(struct_def)),
            Definition::Function(Arc::new(func)),
        ]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(
            expr.ty(),
            &RT::Struct(structured_agent_runtime::symbols::TypeName::new(
                structured_agent_runtime::symbols::ModuleName::new(nonempty::NonEmpty::new(
                    "main".to_string()
                )),
                "Point",
            ))
        );
    }

    #[test]
    fn field_access_has_field_type() {
        let struct_def = crate::ast::StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            fields: vec![
                crate::ast::StructField {
                    name: "x".to_string(),
                    field_type: AstType::simple("Int"),
                    span: crate::types::Span::dummy(),
                },
                crate::ast::StructField {
                    name: "y".to_string(),
                    field_type: AstType::simple("Int"),
                    span: crate::types::Span::dummy(),
                },
            ],
            span: crate::types::Span::dummy(),
        };
        let func = create_test_function(
            "f",
            vec![create_parameter("p", AstType::simple("Point"))],
            AstType::simple("Int"),
            vec![Statement::Return(Expression::FieldAccess {
                base: Box::new(Expression::Variable {
                    name: "p".to_string(),
                    span: crate::types::Span::dummy(),
                }),
                field: "x".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Struct(Arc::new(struct_def)),
            Definition::Function(Arc::new(func)),
        ]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::int());
    }

    #[test]
    fn select_expression_has_branch_type() {
        let callee = create_test_function(
            "get_str",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringLiteral {
                value: "v".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::Select(SelectExpression {
                clauses: vec![SelectClause {
                    expression_to_run: Expression::Call {
                        function: "get_str".to_string(),
                        arguments: vec![],
                        span: crate::types::Span::dummy(),
                    },
                    result_variable: "s".to_string(),
                    expression_next: Expression::Variable {
                        name: "s".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            }))],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(callee)),
            Definition::Function(Arc::new(func)),
        ]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "f" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = stmt_expr(f.body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::string());
        assert!(matches!(expr, typed_ast::Expression::Select(_, _)));
    }

    #[test]
    fn generic_head_with_string_list_returns_option_string() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType {
                    name: "List".to_string(),
                    args: vec![AstType::simple("T")],
                },
            )],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("T")],
            },
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![create_parameter(
                "xs",
                AstType {
                    name: "List".to_string(),
                    args: vec![AstType::simple("String")],
                },
            )],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("String")],
            },
            vec![Statement::Return(Expression::Call {
                function: "head".to_string(),
                arguments: vec![Expression::Variable {
                    name: "xs".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(head)),
            Definition::Function(Arc::new(caller)),
        ]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "f" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = stmt_expr(f.body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::option(RT::string()));
    }

    #[test]
    fn generic_head_with_int_list_returns_option_int() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType {
                    name: "List".to_string(),
                    args: vec![AstType::simple("T")],
                },
            )],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("T")],
            },
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![create_parameter(
                "xs",
                AstType {
                    name: "List".to_string(),
                    args: vec![AstType::simple("Int")],
                },
            )],
            AstType {
                name: "Option".to_string(),
                args: vec![AstType::simple("Int")],
            },
            vec![Statement::Return(Expression::Call {
                function: "head".to_string(),
                arguments: vec![Expression::Variable {
                    name: "xs".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(head)),
            Definition::Function(Arc::new(caller)),
        ]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "f" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = stmt_expr(f.body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::option(RT::int()));
    }

    #[test]
    fn generic_zip_with_two_type_params_typechecks() {
        let zip = create_generic_test_function(
            "zip",
            vec!["A".into(), "B".into()],
            vec![
                create_parameter(
                    "a",
                    AstType {
                        name: "List".to_string(),
                        args: vec![AstType::simple("A")],
                    },
                ),
                create_parameter(
                    "b",
                    AstType {
                        name: "List".to_string(),
                        args: vec![AstType::simple("B")],
                    },
                ),
            ],
            AstType {
                name: "List".to_string(),
                args: vec![AstType::simple("A")],
            },
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![
                create_parameter(
                    "strs",
                    AstType {
                        name: "List".to_string(),
                        args: vec![AstType::simple("String")],
                    },
                ),
                create_parameter(
                    "ints",
                    AstType {
                        name: "List".to_string(),
                        args: vec![AstType::simple("Int")],
                    },
                ),
            ],
            AstType {
                name: "List".to_string(),
                args: vec![AstType::simple("String")],
            },
            vec![Statement::Return(Expression::Call {
                function: "zip".to_string(),
                arguments: vec![
                    Expression::Variable {
                        name: "strs".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                    Expression::Variable {
                        name: "ints".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                ],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(zip)),
            Definition::Function(Arc::new(caller)),
        ]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "f" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = stmt_expr(f.body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::list(RT::string()));
    }

    #[test]
    fn external_call_carries_external_kind() {
        let ext = crate::ast::ExternalFunction {
            name: "native_fn".to_string(),
            type_params: vec![],
            parameters: vec![],
            return_type: AstType::simple("Int"),
            is_pub: false,
            span: crate::types::Span::dummy(),
        };
        let caller = create_test_function(
            "f",
            vec![],
            AstType::simple("Int"),
            vec![Statement::Return(Expression::Call {
                function: "native_fn".to_string(),
                arguments: vec![],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::ExternalFunction(Arc::new(ext)),
            Definition::Function(Arc::new(caller)),
        ]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "f" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = stmt_expr(f.body.statements.first().unwrap());
        assert!(matches!(
            expr,
            typed_ast::Expression::Call {
                kind: FunctionKind::External,
                ..
            }
        ));
    }

    #[test]
    #[ignore = "requires witness table dispatch which is not yet implemented"]
    fn test_impl_function_call_resolves_to_qualified_name() {
        let input = concat!(
            "struct Vec2 {\n",
            "    x: Int,\n",
            "    y: Int,\n",
            "}\n",
            "trait Add {\n",
            "    fn add(self: Self, other: Self): Self\n",
            "}\n",
            "impl Vec2: Add {\n",
            "    fn add(self: Vec2, other: Vec2): Vec2 {\n",
            "        return self\n",
            "    }\n",
            "}\n",
            "fn main(): Vec2 {\n",
            "    let v1 = Vec2 { x: 1, y: 2 }\n",
            "    let v2 = Vec2 { x: 3, y: 4 }\n",
            "    return add(v1, v2)\n",
            "}\n"
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let typed_module = check_typed(&module);
        let main_fn = typed_module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "main" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let resolved = main_fn
            .body
            .statements
            .iter()
            .find_map(|s| {
                if let typed_ast::Statement::Return(typed_ast::Expression::Call {
                    resolved, ..
                }) = s
                {
                    Some(resolved.clone())
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(resolved, {
            let mn = ModuleName::new(NonEmpty::new("main".to_string()));
            let key = ImplKey::new(mn, Some(0));
            FunctionName::for_impl(&key, "add")
        });
    }

    #[test]
    fn generic_struct_literal_has_parameterized_type() {
        let input = concat!(
            "struct Box<T> {\n",
            "    value: T,\n",
            "}\n",
            "fn make_box(): Box<String> {\n",
            "    return Box { value: \"hello\" }\n",
            "}\n"
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let typed_module = check_typed(&module);
        let make_box_fn = typed_module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "make_box" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = make_box_fn
            .body
            .statements
            .iter()
            .find_map(|s| {
                if let typed_ast::Statement::Return(e) = s {
                    Some(e)
                } else {
                    None
                }
            })
            .unwrap();
        let mn = ModuleName::new(NonEmpty::new("main".to_string()));
        assert_eq!(
            expr.ty(),
            &RT::Parameterized(
                structured_agent_runtime::symbols::TypeName::new(mn, "Box"),
                vec![RT::string()],
            )
        );
    }
}

mod metadata_query_tests {
    use super::*;
    use crate::ast::{AstSignature, SigFunction, StructDefinition, StructField};
    use crate::typecheck::{CheckerAstRef, TypedCheckerAstRef, TypedRefs};
    use nonempty::NonEmpty;
    use std::sync::Arc;
    use structured_agent_runtime::symbols::{
        FunctionName, MetaData, ModuleName, SymbolQuery, TypeDefinitionKind, TypeName,
    };

    fn check_meta(module: crate::ast::Module) -> MetaData<TypedRefs> {
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("main".to_string()),
            module,
            is_entry: true,
            file_id: 0,
            is_inline: false,
        };
        let metadata = TypeChecker::new()
            .check(&[parsed], &std::collections::HashMap::new())
            .unwrap();
        metadata
    }

    #[test]
    fn metadata_module_is_populated() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "main",
                vec![],
                AstType::simple("Unit"),
                vec![Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .module(&ModuleName::new(NonEmpty::new("main".to_string())))
                .is_some()
        );
    }

    #[test]
    fn metadata_function_is_queryable() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "greet",
                vec![create_parameter("name", AstType::simple("String"))],
                AstType::simple("String"),
                vec![Statement::Return(Expression::Variable {
                    name: "name".to_string(),
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .function(&FunctionName::new(
                    ModuleName::new(NonEmpty::new("main".to_string())),
                    "greet",
                ))
                .is_some()
        );
    }

    #[test]
    fn metadata_struct_type_is_queryable() {
        let module = create_test_module(vec![Definition::Struct(Arc::new(StructDefinition {
            name: "Point".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                name: "x".to_string(),
                field_type: AstType::simple("Int"),
                span: crate::types::Span::dummy(),
            }],
            span: crate::types::Span::dummy(),
        }))]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&TypeName::new(
                    ModuleName::new(NonEmpty::new("main".to_string())),
                    "Point",
                ))
                .is_some()
        );
    }

    #[test]
    fn metadata_trait_is_queryable() {
        let module = create_test_module(vec![Definition::Trait(std::sync::Arc::new(AstTrait {
            name: "Greetable".to_string(),
            functions: vec![SigFunction {
                name: "greet".to_string(),
                type_params: vec![],
                parameters: vec![],
                return_type: AstType::simple("String"),
                span: crate::types::Span::dummy(),
            }],
            span: crate::types::Span::dummy(),
        }))]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&TypeName::new(
                    ModuleName::new(NonEmpty::new("main".to_string())),
                    "Greetable",
                ))
                .is_some()
        );
    }

    #[test]
    fn metadata_impl_is_queryable() {
        let input = concat!(
            "struct Foo {\n",
            "    x: Int,\n",
            "}\n",
            "trait Add {\n",
            "    fn add(self: Foo, other: Foo): Foo\n",
            "}\n",
            "impl Foo: Add {\n",
            "    fn add(self: Foo, other: Foo): Foo {\n",
            "        return self\n",
            "    }\n",
            "}\n",
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("main".to_string()),
            module,
            is_inline: false,
            is_entry: true,
            file_id: 0,
        };
        let metadata = TypeChecker::new()
            .check(&[parsed], &std::collections::HashMap::new())
            .unwrap();
        let impl_def = metadata
            .impls
            .values()
            .find(|v| v.type_name.name() == "Foo" && v.trait_name.name() == "Add");
        assert!(impl_def.is_some());
        assert!(matches!(
            impl_def.unwrap().ast_ref,
            TypedCheckerAstRef::Other(CheckerAstRef::Impl(_))
        ));
    }

    #[test]
    fn prelude_unit_is_in_symbol_table() {
        let module = create_test_module(vec![]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&TypeName::new(
                    ModuleName::new(NonEmpty::new("prelude".to_string())),
                    "Unit",
                ))
                .is_some()
        );
    }

    #[test]
    fn prelude_string_is_in_symbol_table() {
        let module = create_test_module(vec![]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&TypeName::new(
                    ModuleName::new(NonEmpty::new("prelude".to_string())),
                    "String",
                ))
                .is_some()
        );
    }

    #[test]
    fn prelude_list_is_in_symbol_table() {
        let module = create_test_module(vec![]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&TypeName::new(
                    ModuleName::new(NonEmpty::new("prelude".to_string())),
                    "List",
                ))
                .is_some()
        );
    }

    #[test]
    fn list_return_type_resolves_to_prelude_list() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "get_items",
                vec![],
                AstType {
                    name: "List".to_string(),
                    args: vec![AstType::simple("Int")],
                },
                vec![Statement::Return(Expression::ListLiteral {
                    elements: vec![Expression::IntLiteral {
                        value: 1,
                        span: crate::types::Span::dummy(),
                    }],
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let metadata = check_meta(module);
        let fn_def = metadata
            .function(&FunctionName::new(
                ModuleName::new(NonEmpty::new("main".to_string())),
                "get_items",
            ))
            .unwrap();
        assert_eq!(
            fn_def.type_name,
            TypeName::new(
                ModuleName::new(NonEmpty::new("main".to_string())),
                "get_items",
            )
        );
        let type_def = metadata.type_def(&fn_def.type_name).unwrap();
        if let TypeDefinitionKind::Function { return_type, .. } = &type_def.kind {
            assert_eq!(
                return_type,
                &TypeName::new(
                    ModuleName::new(NonEmpty::new("prelude".to_string())),
                    "List",
                )
            );
        } else {
            panic!("expected TypeDefinitionKind::Function");
        }
    }

    #[test]
    fn bytecode_function_type_def_is_queryable() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "greet",
                vec![create_parameter("name", AstType::simple("String"))],
                AstType::simple("String"),
                vec![Statement::Return(Expression::Variable {
                    name: "name".to_string(),
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let metadata = check_meta(module);
        let fn_def = metadata
            .function(&FunctionName::new(
                ModuleName::new(NonEmpty::new("main".to_string())),
                "greet",
            ))
            .unwrap();
        let type_def = metadata.type_def(&fn_def.type_name).unwrap();
        let TypeDefinitionKind::Function {
            parameters,
            return_type,
            ..
        } = &type_def.kind
        else {
            panic!("expected TypeDefinitionKind::Function");
        };
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].name, "name");
        assert_eq!(
            parameters[0].type_name,
            TypeName::new(
                ModuleName::new(NonEmpty::new("prelude".to_string())),
                "String",
            )
        );
        assert_eq!(
            return_type,
            &TypeName::new(
                ModuleName::new(NonEmpty::new("prelude".to_string())),
                "String",
            )
        );
    }

    #[test]
    fn external_function_type_def_is_queryable() {
        use crate::ast::ExternalFunction;
        let ext_func = ExternalFunction {
            name: "math::add".to_string(),
            type_params: vec![],
            parameters: vec![
                create_parameter("a", AstType::simple("Int")),
                create_parameter("b", AstType::simple("Int")),
            ],
            return_type: AstType::simple("Int"),
            is_pub: true,
            span: crate::types::Span::dummy(),
        };
        let module = create_test_module(vec![Definition::ExternalFunction(Arc::new(ext_func))]);
        let metadata = check_meta(module);
        let fn_def = metadata
            .function(&FunctionName::new(
                ModuleName::new(NonEmpty::new("main".to_string())),
                "math::add",
            ))
            .unwrap();
        let type_def = metadata.type_def(&fn_def.type_name).unwrap();
        let TypeDefinitionKind::Function {
            parameters,
            return_type,
            ..
        } = &type_def.kind
        else {
            panic!("expected TypeDefinitionKind::Function");
        };
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "a");
        assert_eq!(parameters[1].name, "b");
        assert_eq!(
            return_type,
            &TypeName::new(ModuleName::new(NonEmpty::new("prelude".to_string())), "Int",)
        );
    }
}
