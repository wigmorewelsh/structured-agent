use super::*;
use combine::{Parser, stream::position::IndexPositioner};
use structured_agent_ast::ast::{
    AstTrait, Definition, Expression, Function, FunctionBody, Module, Parameter, SelectClause,
    SelectExpression, Statement, Type as AstType,
};
use structured_agent_parser::parse_program;

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

fn native_prelude_modules()
-> std::collections::HashMap<String, std::sync::Arc<dyn structured_agent_il::Module>> {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "prelude".to_string(),
        std::sync::Arc::new(structured_agent_stdlib::prelude::PreludeModule)
            as std::sync::Arc<dyn structured_agent_il::Module>,
    );
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use nonempty::NonEmpty;
    use std::sync::Arc;

    fn check(module: crate::ast::Module) -> Result<(), Vec<crate::TypeError>> {
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: true,
            file_id: 0,
            is_inline: false,
        };
        TypeChecker::new()
            .check(&[parsed], &native_prelude_modules())
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
                type_args: vec![],
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
                type_args: vec![],
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
                type_args: vec![],
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
                type_args: vec![],
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
                        type_args: vec![],
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
                            type_args: vec![],
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
                        span: crate::types::Span::dummy(),
                    },
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "concat".to_string(),
                            type_args: vec![],
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
                            type_args: vec![],
                            arguments: vec![],
                            span: crate::types::Span::dummy(),
                        },
                        span: crate::types::Span::dummy(),
                    },
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "get_bool".to_string(),
                            type_args: vec![],
                            arguments: vec![],
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
                type_args: vec![],
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
                    type_args: vec![],
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
                AstType::parameterized("List", vec![AstType::simple("T")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let caller = create_test_function(
            "caller",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType::parameterized("Option", vec![AstType::simple("String")]),
            vec![Statement::Return(Expression::Call {
                function: "head".to_string(),
                type_args: vec![],
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
            return_type: AstType::parameterized("Option", vec![AstType::simple("T")]),
            is_pub: false,
            span: crate::types::Span::dummy(),
        };
        let caller = create_test_function(
            "main",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType::parameterized("Option", vec![AstType::simple("String")]),
            vec![Statement::Return(Expression::Call {
                function: "wrap".to_string(),
                type_args: vec![],
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
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "make_none".to_string(),
                type_args: vec![],
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
                AstType::parameterized("List", vec![AstType::simple("T")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let consume = create_test_function(
            "consume",
            vec![create_parameter(
                "item",
                AstType::parameterized("Option", vec![AstType::simple("String")]),
            )],
            AstType::simple("Unit"),
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![create_parameter(
                "xs",
                AstType::parameterized("List", vec![AstType::simple("String")]),
            )],
            AstType::simple("Unit"),
            vec![
                Statement::Assignment {
                    variable: "result".to_string(),
                    expression: Expression::Call {
                        function: "head".to_string(),
                        type_args: vec![],
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
                    type_args: vec![],
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
        let pair_string = AstType::parameterized("Pair", vec![AstType::simple("String")]);
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

    #[test]
    fn test_inherent_method_resolves_when_trait_impl_also_present() {
        let input = concat!(
            "struct Foo {}\n",
            "trait Bar {\n",
            "    fn bar_fn(self: Self): String\n",
            "}\n",
            "impl Foo {\n",
            "    pub fn get(self): String {\n",
            "        return \"ok\"\n",
            "    }\n",
            "}\n",
            "impl Foo: Bar {\n",
            "    fn bar_fn(self: Foo): String {\n",
            "        return \"ok\"\n",
            "    }\n",
            "}\n",
            "fn main(): String {\n",
            "    let f = Foo {}\n",
            "    return f.get()\n",
            "}\n",
        );
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
            "inherent method should resolve even when a trait impl is also present: {:?}",
            result
        );
    }

    #[test]
    fn test_trait_method_is_callable_on_concrete_type() {
        let input = concat!(
            "struct Foo {}\n",
            "trait Bar {\n",
            "    fn bar_fn(self: Self): String\n",
            "}\n",
            "impl Foo: Bar {\n",
            "    fn bar_fn(self: Foo): String {\n",
            "        return \"ok\"\n",
            "    }\n",
            "}\n",
            "fn main(): String {\n",
            "    let f = Foo {}\n",
            "    return f.bar_fn()\n",
            "}\n",
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        assert!(
            check(module).is_ok(),
            "trait method should be callable via method dispatch on a concrete type"
        );
    }

    #[test]
    fn test_method_call_type_checks() {
        let input = concat!(
            "struct Foo {\n",
            "    x: Int,\n",
            "}\n",
            "impl Foo {\n",
            "    pub fn get(self): Int {\n",
            "        return self.x\n",
            "    }\n",
            "}\n",
            "fn main(): Int {\n",
            "    let foo = Foo { x: 42 }\n",
            "    return foo.get()\n",
            "}\n",
        );
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
            "method call should type check: {:?}",
            result
        );
    }

    #[test]
    fn test_method_call_unknown_method_is_error() {
        let input = concat!(
            "struct Foo {\n",
            "    x: Int,\n",
            "}\n",
            "impl Foo {\n",
            "    pub fn get(self): Int {\n",
            "        return self.x\n",
            "    }\n",
            "}\n",
            "fn main(): Int {\n",
            "    let foo = Foo { x: 42 }\n",
            "    return foo.nonexistent()\n",
            "}\n",
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let result = check(module);
        assert!(
            result.is_err(),
            "unknown method should produce a type error"
        );
    }

    #[test]
    fn test_impl_method_body_type_error_is_reported() {
        let input = concat!(
            "struct Foo {\n",
            "    x: Int,\n",
            "}\n",
            "impl Foo {\n",
            "    pub fn get(self): String {\n",
            "        return self.x\n",
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
        let result = check(module);
        assert!(
            result.is_err(),
            "return type mismatch in impl body should be a type error, got {:?}",
            result
        );
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, TypeError::ReturnTypeMismatch { .. })),
            "expected ReturnTypeMismatch, got {:?}",
            errors
        );
    }

    #[test]
    fn test_impl_fn_with_self_typed_parameter_type_checks() {
        let input = concat!(
            "struct Foo {\n",
            "    x: Int,\n",
            "}\n",
            "impl Foo {\n",
            "    pub fn combine(self: Self, other: Self): Int {\n",
            "        return other.x\n",
            "    }\n",
            "}\n",
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let result = check(module);
        assert!(
            result.is_ok(),
            "impl fn with Self-typed parameter should type check: {:?}",
            result
        );
    }

    #[test]
    fn test_impl_fn_with_self_return_type_type_checks() {
        let input = concat!(
            "struct Foo {\n",
            "    x: Int,\n",
            "}\n",
            "impl Foo {\n",
            "    pub fn clone_self(self: Self): Self {\n",
            "        return self\n",
            "    }\n",
            "}\n",
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let result = check(module);
        assert!(
            result.is_ok(),
            "impl fn with Self return type should type check: {:?}",
            result
        );
    }

    #[test]
    fn method_call_on_type_from_another_module_resolves() {
        let lib_src = concat!(
            "struct Counter {\n",
            "    value: Int,\n",
            "}\n",
            "impl Counter {\n",
            "    pub fn get(self): Int {\n",
            "        return self.value\n",
            "    }\n",
            "}\n",
        );
        let app_src = concat!(
            "use lib::Counter\n",
            "fn main(c: Counter): Int {\n",
            "    return c.get()\n",
            "}\n",
        );
        let parse = |src: &str| {
            parse_program(0)
                .parse(combine::stream::position::Stream::with_positioner(
                    src,
                    IndexPositioner::default(),
                ))
                .unwrap()
                .0
        };
        let lib_module = crate::ast::ParsedModule {
            name: NonEmpty::new("lib".to_string()),
            module: parse(lib_src),
            is_entry: false,
            file_id: 0,
            is_inline: false,
        };
        let app_module = crate::ast::ParsedModule {
            name: NonEmpty::new("app".to_string()),
            module: parse(app_src),
            is_entry: true,
            file_id: 1,
            is_inline: false,
        };
        let metadata = TypeChecker::new()
            .check(&[lib_module, app_module], &native_prelude_modules())
            .unwrap();
        let main_fn = metadata
            .functions
            .values()
            .find(|f| f.name.last_name() == "main" && f.name.module_prefix().to_string() == "app")
            .unwrap();
        let TypedCheckerAstRef::Function(f, _) = &main_fn.ast_ref else {
            panic!("expected Function");
        };
        let return_expr = f
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
        match return_expr {
            typed_ast::Expression::Call {
                binding: typed_ast::MethodBinding::Early(path),
                ..
            } => {
                assert_eq!(path.module_prefix().to_string(), "lib");
                assert_eq!(path.last_name(), "get");
            }
            other => panic!("expected Early binding, got {:?}", other),
        }
    }

    #[test]
    fn trait_method_dispatch_across_modules() {
        let module_a_src = concat!(
            "trait Greet {\n",
            "    fn greet(self: Self): String\n",
            "}\n",
        );
        let module_b_src = concat!(
            "use a::Greet\n",
            "struct Person {\n",
            "    name: String,\n",
            "}\n",
            "impl Person: Greet {\n",
            "    fn greet(self: Person): String {\n",
            "        return self.name\n",
            "    }\n",
            "}\n",
        );
        let module_c_src = concat!(
            "use b::Person\n",
            "pub fn describe(p: Person): String {\n",
            "    return p.greet()\n",
            "}\n",
        );
        let main_src = concat!(
            "use b::Person\n",
            "use c::describe\n",
            "fn main(p: Person): String {\n",
            "    return describe(p)\n",
            "}\n",
        );
        let parse = |src: &str| {
            parse_program(0)
                .parse(combine::stream::position::Stream::with_positioner(
                    src,
                    IndexPositioner::default(),
                ))
                .unwrap()
                .0
        };
        let modules = vec![
            crate::ast::ParsedModule {
                name: NonEmpty::new("a".to_string()),
                module: parse(module_a_src),
                is_entry: false,
                file_id: 0,
                is_inline: false,
            },
            crate::ast::ParsedModule {
                name: NonEmpty::new("b".to_string()),
                module: parse(module_b_src),
                is_entry: false,
                file_id: 1,
                is_inline: false,
            },
            crate::ast::ParsedModule {
                name: NonEmpty::new("c".to_string()),
                module: parse(module_c_src),
                is_entry: false,
                file_id: 2,
                is_inline: false,
            },
            crate::ast::ParsedModule {
                name: NonEmpty::new("main".to_string()),
                module: parse(main_src),
                is_entry: true,
                file_id: 3,
                is_inline: false,
            },
        ];
        let metadata = TypeChecker::new()
            .check(&modules, &native_prelude_modules())
            .unwrap();
        let describe_fn = metadata
            .functions
            .values()
            .find(|f| f.name.last_name() == "describe" && f.name.module_prefix().to_string() == "c")
            .unwrap();
        let TypedCheckerAstRef::Function(f, _) = &describe_fn.ast_ref else {
            panic!("expected Function");
        };
        let return_expr = f
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
        match return_expr {
            typed_ast::Expression::Call {
                binding: typed_ast::MethodBinding::Early(path),
                ..
            } => {
                assert_eq!(path.module_prefix().to_string(), "b");
                assert_eq!(path.last_name(), "greet");
            }
            other => panic!("expected Early binding into module b, got {:?}", other),
        }
    }

    #[test]
    fn generic_trait_bound_dispatch_across_modules() {
        let module_a_src = concat!(
            "trait Greet {\n",
            "    fn greet(self: Self): String\n",
            "}\n",
        );
        let module_b_src = concat!(
            "use a::Greet\n",
            "struct Person {\n",
            "    name: String,\n",
            "}\n",
            "impl Person: Greet {\n",
            "    fn greet(self: Person): String {\n",
            "        return self.name\n",
            "    }\n",
            "}\n",
        );
        let module_c_src = concat!(
            "use a::Greet\n",
            "pub fn describe<T: Greet>(p: T): String {\n",
            "    return p.greet()\n",
            "}\n",
        );
        let main_src = concat!(
            "use a::Greet\n",
            "use b::Person\n",
            "use c::describe\n",
            "fn main(p: Person): String {\n",
            "    return describe(p)\n",
            "}\n",
        );
        let parse = |src: &str| {
            parse_program(0)
                .parse(combine::stream::position::Stream::with_positioner(
                    src,
                    IndexPositioner::default(),
                ))
                .unwrap()
                .0
        };
        let modules = vec![
            crate::ast::ParsedModule {
                name: NonEmpty::new("a".to_string()),
                module: parse(module_a_src),
                is_entry: false,
                file_id: 0,
                is_inline: false,
            },
            crate::ast::ParsedModule {
                name: NonEmpty::new("b".to_string()),
                module: parse(module_b_src),
                is_entry: false,
                file_id: 1,
                is_inline: false,
            },
            crate::ast::ParsedModule {
                name: NonEmpty::new("c".to_string()),
                module: parse(module_c_src),
                is_entry: false,
                file_id: 2,
                is_inline: false,
            },
            crate::ast::ParsedModule {
                name: NonEmpty::new("main".to_string()),
                module: parse(main_src),
                is_entry: true,
                file_id: 3,
                is_inline: false,
            },
        ];
        let metadata = TypeChecker::new()
            .check(&modules, &native_prelude_modules())
            .unwrap();
        let describe_fn = metadata
            .functions
            .values()
            .find(|f| f.name.last_name() == "describe" && f.name.module_prefix().to_string() == "c")
            .unwrap();
        let TypedCheckerAstRef::Function(describe_f, _) = &describe_fn.ast_ref else {
            panic!("expected Function");
        };
        let describe_return = describe_f
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
        match describe_return {
            typed_ast::Expression::Call {
                binding: typed_ast::MethodBinding::Late(_, path),
                ..
            } => {
                assert_eq!(path.last_name(), "greet");
            }
            other => panic!(
                "expected Late binding in generic describe body, got {:?}",
                other
            ),
        }
        let main_fn = metadata
            .functions
            .values()
            .find(|f| f.name.last_name() == "main" && f.name.module_prefix().to_string() == "main")
            .unwrap();
        let TypedCheckerAstRef::Function(main_f, _) = &main_fn.ast_ref else {
            panic!("expected Function");
        };
        let main_return = main_f
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
        match main_return {
            typed_ast::Expression::Call { arguments, .. } => {
                let impl_arg = arguments
                    .iter()
                    .find(|a| matches!(a, typed_ast::Expression::ModuleInstance { .. }));
                let typed_ast::Expression::ModuleInstance { path, .. } =
                    impl_arg.expect("expected ModuleInstance arg for impl witness")
                else {
                    unreachable!()
                };
                assert_eq!(path.module_prefix().to_string(), "b");
            }
            other => panic!("expected Call in main return, got {:?}", other),
        }
    }

    #[test]
    fn use_relative_path_resolves() {
        let io_src = "struct Msg {\n    value: String,\n}\n";
        let app_src = "use io::Msg\nfn main(m: Msg): Unit {\n    return ()\n}\n";
        let parse = |src: &str| {
            parse_program(0)
                .parse(combine::stream::position::Stream::with_positioner(
                    src,
                    IndexPositioner::default(),
                ))
                .unwrap()
                .0
        };
        let io_module = crate::ast::ParsedModule {
            name: NonEmpty::new("io".to_string()),
            module: parse(io_src),
            is_entry: false,
            file_id: 0,
            is_inline: false,
        };
        let app_module = crate::ast::ParsedModule {
            name: NonEmpty::new("app".to_string()),
            module: parse(app_src),
            is_entry: true,
            file_id: 1,
            is_inline: false,
        };
        let result = TypeChecker::new()
            .check(&[io_module, app_module], &native_prelude_modules())
            .map(|_| ());
        assert!(
            result.is_ok(),
            "relative use path should resolve: {:?}",
            result
        );
    }

    #[test]
    fn use_absolute_path_resolves() {
        let io_src = "struct Msg {\n    value: String,\n}\n";
        let app_src = "use ::io::Msg\nfn main(m: Msg): Unit {\n    return ()\n}\n";
        let parse = |src: &str| {
            parse_program(0)
                .parse(combine::stream::position::Stream::with_positioner(
                    src,
                    IndexPositioner::default(),
                ))
                .unwrap()
                .0
        };
        let io_module = crate::ast::ParsedModule {
            name: NonEmpty::new("io".to_string()),
            module: parse(io_src),
            is_entry: false,
            file_id: 0,
            is_inline: false,
        };
        let app_module = crate::ast::ParsedModule {
            name: NonEmpty::new("app".to_string()),
            module: parse(app_src),
            is_entry: true,
            file_id: 1,
            is_inline: false,
        };
        let result = TypeChecker::new()
            .check(&[io_module, app_module], &native_prelude_modules())
            .map(|_| ());
        assert!(
            result.is_ok(),
            "absolute use path should resolve: {:?}",
            result
        );
    }

    #[test]
    fn use_absolute_path_nonexistent_module_fails() {
        let app_src = "use ::nonexistent::Foo\nfn main(f: Foo): Unit {\n    return ()\n}\n";
        let parse = |src: &str| {
            parse_program(0)
                .parse(combine::stream::position::Stream::with_positioner(
                    src,
                    IndexPositioner::default(),
                ))
                .unwrap()
                .0
        };
        let app_module = crate::ast::ParsedModule {
            name: NonEmpty::new("app".to_string()),
            module: parse(app_src),
            is_entry: true,
            file_id: 0,
            is_inline: false,
        };
        let result = TypeChecker::new()
            .check(&[app_module], &native_prelude_modules())
            .map(|_| ());
        assert!(result.is_err(), "nonexistent module should fail to resolve");
    }

    fn check_with_iterator(module: crate::ast::Module) -> Result<(), Vec<crate::TypeError>> {
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: true,
            file_id: 0,
            is_inline: false,
        };
        let mut native = native_prelude_modules();
        native.insert(
            "iterator".to_string(),
            Arc::new(structured_agent_stdlib::iterator::IteratorModule)
                as Arc<dyn structured_agent_il::Module>,
        );
        TypeChecker::new().check(&[parsed], &native).map(|_| ())
    }

    #[test]
    fn for_in_rejects_non_iterator_expression() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("Unit"),
            vec![Statement::ForIn {
                variable: "x".to_string(),
                iterable: Expression::StringLiteral {
                    value: "hello".to_string(),
                    span: crate::types::Span::dummy(),
                },
                body: vec![],
                span: crate::types::Span::dummy(),
            }],
        );
        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);
        let result = check_with_iterator(module);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, crate::TypeError::TraitBoundNotSatisfied { trait_name, .. } if trait_name == "Iterator")));
    }

    #[test]
    fn for_in_loop_variable_not_visible_after_loop() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::simple("String"),
            vec![
                Statement::ForIn {
                    variable: "x".to_string(),
                    iterable: Expression::StringLiteral {
                        value: "hello".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                    body: vec![],
                    span: crate::types::Span::dummy(),
                },
                Statement::Return(Expression::Variable {
                    name: "x".to_string(),
                    span: crate::types::Span::dummy(),
                }),
            ],
        );
        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);
        let result = check_with_iterator(module);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod typed_ast_tests {
    use super::*;
    use crate::typed_ast;
    use crate::{FunctionKind, TypedCheckerAstRef};
    use nonempty::NonEmpty;

    use std::sync::Arc;
    use structured_agent_ast::ast::StringPart;
    use structured_agent_runtime::Type as RT;
    use structured_agent_runtime::symbols::DefinitionPath;

    fn check_typed(module: &Module) -> typed_ast::Module {
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("main".to_string()),
            module: module.clone(),
            is_entry: false,
            file_id: 0,
            is_inline: false,
        };
        let typed_metadata = TypeChecker::new()
            .check(&[parsed], &native_prelude_modules())
            .unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module_prefix().to_string() != "main" {
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
            fn test_func<T: Int>(x: T): T { return x }
            fn main(): () { test_func(42) }
        "#;
        let stream = combine::stream::position::Stream::with_positioner(
            code,
            combine::stream::position::IndexPositioner::default(),
        );
        let (module, _) = parse_program(0).parse(stream).unwrap();
        let parsed = crate::ast::ParsedModule {
            name: nonempty::NonEmpty::new("test".to_string()),
            is_entry: false,
            is_inline: false,
            file_id: 0,
            module,
        };
        let mut checker = super::TypeChecker::new();
        let metadata = checker.check(&[parsed], &native_prelude_modules()).unwrap();

        let main_fn = metadata
            .functions
            .values()
            .find(|f| f.name.last_name() == "main")
            .unwrap();
        let TypedCheckerAstRef::Function(f, _) = &main_fn.ast_ref else {
            panic!()
        };

        let stmt = f.body.statements.first().unwrap();
        let crate::typed_ast::Statement::ExpressionStatement(crate::typed_ast::Expression::Call {
            arguments,
            ..
        }) = stmt
        else {
            panic!()
        };

        assert_eq!(arguments.len(), 2);
        assert!(
            matches!(
                &arguments[0],
                crate::typed_ast::Expression::TypeLiteral { .. }
            ),
            "expected TypeLiteral as first arg, got {:?}",
            &arguments[0]
        );
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
                type_args: vec![],
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
        let expected_path = DefinitionPath::for_function(
            DefinitionPath::for_module(NonEmpty::new("main".to_string())),
            "get_value",
        );
        assert!(matches!(
            expr,
            typed_ast::Expression::Call { binding: typed_ast::MethodBinding::Early(path), kind: FunctionKind::Bytecode, .. }
            if path == &expected_path
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
                type_args: vec![],
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
            AstType::parameterized("List", vec![AstType::simple("Int")]),
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
            &RT::Named(DefinitionPath::for_type(
                DefinitionPath::for_module(nonempty::NonEmpty::new("main".to_string())),
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
                        type_args: vec![],
                        arguments: vec![],
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
                AstType::parameterized("List", vec![AstType::simple("T")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![create_parameter(
                "xs",
                AstType::parameterized("List", vec![AstType::simple("String")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("String")]),
            vec![Statement::Return(Expression::Call {
                function: "head".to_string(),
                type_args: vec![],
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
                AstType::parameterized("List", vec![AstType::simple("T")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![create_parameter(
                "xs",
                AstType::parameterized("List", vec![AstType::simple("Int")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("Int")]),
            vec![Statement::Return(Expression::Call {
                function: "head".to_string(),
                type_args: vec![],
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
                    AstType::parameterized("List", vec![AstType::simple("A")]),
                ),
                create_parameter(
                    "b",
                    AstType::parameterized("List", vec![AstType::simple("B")]),
                ),
            ],
            AstType::parameterized("List", vec![AstType::simple("A")]),
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![
                create_parameter(
                    "strs",
                    AstType::parameterized("List", vec![AstType::simple("String")]),
                ),
                create_parameter(
                    "ints",
                    AstType::parameterized("List", vec![AstType::simple("Int")]),
                ),
            ],
            AstType::parameterized("List", vec![AstType::simple("String")]),
            vec![Statement::Return(Expression::Call {
                function: "zip".to_string(),
                type_args: vec![],
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
                type_args: vec![],
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
    fn generic_function_body_has_late_binding_for_trait_method() {
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
            "fn combine<T: Add>(a: T, b: T): T {\n",
            "    return a.add(b)\n",
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
        let combine_fn = typed_module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "combine" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(
            combine_fn.parameters[0].name, "T",
            "expected type param slot T as first parameter, got {:?}",
            combine_fn.parameters[0].name
        );
        assert!(
            combine_fn.parameters[1].name.starts_with("__T__"),
            "expected implicit param starting with __T__ as second parameter, got {:?}",
            combine_fn.parameters[1].name
        );
        let return_expr = combine_fn
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
        match return_expr {
            typed_ast::Expression::Call {
                binding: typed_ast::MethodBinding::Late(_, path),
                ..
            } => {
                assert_eq!(path.last_name(), "add");
            }
            other => panic!("expected Call with MethodBinding::Late, got {:?}", other),
        }
    }

    #[test]
    fn call_site_of_generic_function_prepends_module_instance() {
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
            "fn combine<T: Add>(a: T, b: T): T {\n",
            "    return a.add(b)\n",
            "}\n",
            "fn main(): Vec2 {\n",
            "    let v = Vec2 { x: 1, y: 2 }\n",
            "    return combine(v, v)\n",
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
        let return_expr = main_fn
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
        match return_expr {
            typed_ast::Expression::Call { arguments, .. } => {
                let mn = DefinitionPath::for_module(NonEmpty::new("main".to_string()));
                let expected_impl = DefinitionPath::for_impl(mn, Some(0));
                assert!(
                    matches!(&arguments[0], typed_ast::Expression::TypeLiteral { .. }),
                    "expected TypeLiteral as first arg, got {:?}",
                    &arguments[0]
                );
                match &arguments[1] {
                    typed_ast::Expression::ModuleInstance { path, .. } => {
                        assert_eq!(path, &expected_impl);
                    }
                    other => panic!("expected ModuleInstance as second arg, got {:?}", other),
                }
            }
            other => panic!("expected Call, got {:?}", other),
        }
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
        let mn = DefinitionPath::for_module(NonEmpty::new("main".to_string()));
        assert_eq!(
            expr.ty(),
            &RT::Parameterized(DefinitionPath::for_type(mn, "Box"), vec![RT::string()],)
        );
    }

    #[test]
    fn binding_ids_are_unique_within_function_and_match_references() {
        let func = create_test_function(
            "f",
            vec![
                create_parameter("x", AstType::simple("Int")),
                create_parameter("y", AstType::simple("Int")),
            ],
            AstType::simple("Int"),
            vec![
                Statement::Assignment {
                    variable: "z".to_string(),
                    expression: Expression::Variable {
                        name: "x".to_string(),
                        span: crate::types::Span::dummy(),
                    },
                    span: crate::types::Span::dummy(),
                },
                Statement::Return(Expression::Variable {
                    name: "z".to_string(),
                    span: crate::types::Span::dummy(),
                }),
            ],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let f = first_function(&module);

        let x_id = f.parameters[0].binding_id;
        let y_id = f.parameters[1].binding_id;

        let (z_id, x_ref_id) = if let typed_ast::Statement::Assignment {
            binding_id,
            expression:
                typed_ast::Expression::Variable {
                    binding_id: ref_id, ..
                },
            ..
        } = f.body.statements[0]
        {
            (binding_id, ref_id)
        } else {
            panic!("expected assignment with variable expression");
        };

        let z_ref_id = if let typed_ast::Statement::Return(typed_ast::Expression::Variable {
            binding_id,
            ..
        }) = f.body.statements[1]
        {
            binding_id
        } else {
            panic!("expected return with variable expression");
        };

        assert_ne!(x_id, y_id);
        assert_ne!(x_id, z_id);
        assert_ne!(y_id, z_id);

        assert_eq!(x_ref_id, x_id);
        assert_eq!(z_ref_id, z_id);
    }

    #[test]
    fn two_functions_have_independent_binding_id_sequences() {
        let func1 = create_test_function(
            "f1",
            vec![create_parameter("a", AstType::simple("Int"))],
            AstType::simple("Int"),
            vec![Statement::Return(Expression::Variable {
                name: "a".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let func2 = create_test_function(
            "f2",
            vec![create_parameter("b", AstType::simple("Int"))],
            AstType::simple("Int"),
            vec![Statement::Return(Expression::Variable {
                name: "b".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(func1)),
            Definition::Function(Arc::new(func2)),
        ]));

        let functions: Vec<_> = module
            .definitions
            .iter()
            .filter_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    Some(f)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(functions.len(), 2);
        for f in &functions {
            let param_id = f.parameters[0].binding_id;
            let ref_id = if let typed_ast::Statement::Return(typed_ast::Expression::Variable {
                binding_id,
                ..
            }) = f.body.statements[0]
            {
                binding_id
            } else {
                panic!("expected return with variable");
            };
            assert_eq!(ref_id, param_id);
        }
    }

    #[test]
    fn test_generic_function_has_type_param_slot() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType::parameterized("List", vec![AstType::simple("T")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            head,
        ))]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "head" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(f.parameters[0].name, "T");
        assert_eq!(f.parameters[0].param_type, RT::Generic("T".to_string()));
        assert_eq!(f.parameters[1].name, "list");
    }

    #[test]
    fn test_call_to_generic_function_prepends_inferred_type_literal() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType::parameterized("List", vec![AstType::simple("T")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![create_parameter(
                "xs",
                AstType::parameterized("List", vec![AstType::simple("String")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("String")]),
            vec![Statement::Return(Expression::Call {
                function: "head".to_string(),
                type_args: vec![],
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
        if let typed_ast::Expression::Call { arguments, .. } = expr {
            assert_eq!(arguments.len(), 2, "expected type arg + value arg");
            assert!(
                matches!(&arguments[0], typed_ast::Expression::TypeLiteral { ty, .. } if *ty == RT::string()),
                "expected TypeLiteral(String) as first arg, got {:?}",
                &arguments[0]
            );
        } else {
            panic!("expected Call expression");
        }
    }

    #[test]
    fn test_call_with_explicit_type_arg_prepends_type_literal() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType::parameterized("List", vec![AstType::simple("T")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![create_parameter(
                "xs",
                AstType::parameterized("List", vec![AstType::simple("String")]),
            )],
            AstType::parameterized("Option", vec![AstType::simple("String")]),
            vec![Statement::Return(Expression::Call {
                function: "head".to_string(),
                type_args: vec![AstType::simple("String")],
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
        if let typed_ast::Expression::Call { arguments, .. } = expr {
            assert_eq!(arguments.len(), 2, "expected type arg + value arg");
            assert!(
                matches!(&arguments[0], typed_ast::Expression::TypeLiteral { ty, .. } if *ty == RT::string()),
                "expected TypeLiteral(String) as first arg, got {:?}",
                &arguments[0]
            );
        } else {
            panic!("expected Call expression");
        }
    }

    #[test]
    fn test_generic_function_passes_type_param_as_variable() {
        let wrap = create_generic_test_function(
            "wrap",
            vec!["T".into()],
            vec![create_parameter("x", AstType::simple("T"))],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let proxy = create_generic_test_function(
            "proxy",
            vec!["T".into()],
            vec![create_parameter("x", AstType::simple("T"))],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![Statement::Return(Expression::Call {
                function: "wrap".to_string(),
                type_args: vec![AstType::simple("T")],
                arguments: vec![Expression::Variable {
                    name: "x".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(wrap)),
            Definition::Function(Arc::new(proxy)),
        ]));
        let f = module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "proxy" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let expr = stmt_expr(f.body.statements.first().unwrap());
        if let typed_ast::Expression::Call { arguments, .. } = expr {
            assert!(
                matches!(&arguments[0], typed_ast::Expression::Variable { name, .. } if name == "T"),
                "expected Variable(T) as first arg, got {:?}",
                &arguments[0]
            );
        } else {
            panic!("expected Call expression");
        }
    }

    #[test]
    fn test_explicit_type_arg_resolves_unresolvable_return_type() {
        let make_none = create_generic_test_function(
            "make_none",
            vec!["T".into()],
            vec![],
            AstType::parameterized("Option", vec![AstType::simple("T")]),
            vec![],
        );
        let main_fn = create_test_function(
            "main",
            vec![],
            AstType::parameterized("Option", vec![AstType::simple("String")]),
            vec![Statement::Return(Expression::Call {
                function: "make_none".to_string(),
                type_args: vec![AstType::simple("String")],
                arguments: vec![],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![
            Definition::Function(Arc::new(make_none)),
            Definition::Function(Arc::new(main_fn)),
        ]));
        let f = module
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
        let expr = stmt_expr(f.body.statements.first().unwrap());
        assert_eq!(expr.ty(), &RT::option(RT::string()));
    }

    #[test]
    fn method_call_placeholder_carries_parameter_type() {
        let input = concat!(
            "struct Foo {}\n",
            "impl Foo {\n",
            "    pub fn process(self: Self, a: String, b: Int): String {\n",
            "        return a\n",
            "    }\n",
            "}\n",
            "fn main(): String {\n",
            "    let f = Foo {}\n",
            "    return f.process(_, 42)\n",
            "}\n",
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
            .expect("main function should be elaborated");
        let expr = main_fn
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
            .expect("expected return statement");
        if let typed_ast::Expression::Call { arguments, .. } = expr {
            let placeholder = arguments
                .iter()
                .find(|a| matches!(a, typed_ast::Expression::Placeholder { .. }));
            let placeholder = placeholder.expect("expected a placeholder argument");
            assert_eq!(
                placeholder.ty(),
                &RT::string(),
                "placeholder should carry String type"
            );
        } else {
            panic!("expected Call expression");
        }
    }

    #[test]
    fn method_call_generic_return_type_resolves() {
        let input = concat!(
            "struct Wrapper {}\n",
            "impl Wrapper {\n",
            "    pub fn identity<T>(self: Self, x: T): T {\n",
            "        return x\n",
            "    }\n",
            "}\n",
            "fn take_int(x: Int): Unit {}\n",
            "fn main(): Unit {\n",
            "    let w = Wrapper {}\n",
            "    let r = w.identity(42)\n",
            "    take_int(r)\n",
            "}\n",
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
            .expect("main function should be elaborated");
        let assign = main_fn
            .body
            .statements
            .iter()
            .find_map(|s| {
                if let typed_ast::Statement::Assignment { expression, .. } = s {
                    if let typed_ast::Expression::Call { function, .. } = expression {
                        if function == "identity" {
                            return Some(expression);
                        }
                    }
                }
                None
            })
            .expect("expected assignment from identity call");
        assert_eq!(
            assign.ty(),
            &RT::int(),
            "identity<T>(42) should resolve return type to Int"
        );
    }

    #[test]
    fn generic_receiver_multiple_bounds_selects_correct_trait() {
        let input = concat!(
            "trait Add {\n",
            "    fn add(self: Self, other: Self): Self\n",
            "}\n",
            "trait Display {\n",
            "    fn display(self: Self): String\n",
            "}\n",
            "fn show<T: Add + Display>(a: T): String {\n",
            "    return a.display()\n",
            "}\n",
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let typed_module = check_typed(&module);
        let show_fn = typed_module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "show" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .expect("show function should be elaborated");
        let return_expr = show_fn
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
            .expect("expected return statement");
        match return_expr {
            typed_ast::Expression::Call {
                binding: typed_ast::MethodBinding::Late(_, path),
                ty,
                ..
            } => {
                assert_eq!(path.last_name(), "display");
                assert!(
                    path.to_string().contains("Display"),
                    "binding path should reference Display trait, got {:?}",
                    path
                );
                assert_eq!(ty, &RT::string(), "return type should be String");
            }
            other => panic!("expected Call with Late binding, got {:?}", other),
        }
    }

    #[test]
    fn generic_receiver_placeholder_carries_parameter_type() {
        let input = concat!(
            "trait Transform {\n",
            "    fn apply(self: Self, x: String): String\n",
            "}\n",
            "fn call_transform<T: Transform>(a: T): String {\n",
            "    return a.apply(_)\n",
            "}\n",
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let typed_module = check_typed(&module);
        let f = typed_module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "call_transform" {
                        Some(f)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .expect("call_transform should be elaborated");
        let return_expr = f
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
            .expect("expected return statement");
        if let typed_ast::Expression::Call { arguments, .. } = return_expr {
            assert_eq!(arguments.len(), 2, "expected receiver + user arg");
            let placeholder = arguments
                .iter()
                .find(|a| matches!(a, typed_ast::Expression::Placeholder { .. }))
                .expect("expected placeholder in arguments");
            assert_eq!(
                placeholder.ty(),
                &RT::string(),
                "placeholder should carry String type"
            );
        } else {
            panic!("expected Call expression");
        }
    }

    #[test]
    fn generic_receiver_call_has_receiver_then_user_args() {
        let input = concat!(
            "trait Add {\n",
            "    fn add(self: Self, other: Self): Self\n",
            "}\n",
            "fn combine<T: Add>(a: T, b: T): T {\n",
            "    return a.add(b)\n",
            "}\n",
        );
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let typed_module = check_typed(&module);
        let combine_fn = typed_module
            .definitions
            .iter()
            .find_map(|d| {
                if let typed_ast::Definition::Function(f) = d {
                    if f.name == "combine" { Some(f) } else { None }
                } else {
                    None
                }
            })
            .expect("combine should be elaborated");
        let return_expr = combine_fn
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
            .expect("expected return statement");
        if let typed_ast::Expression::Call { arguments, .. } = return_expr {
            assert_eq!(arguments.len(), 2, "expected exactly receiver + user arg");
            assert!(
                matches!(&arguments[0], typed_ast::Expression::Variable { name, .. } if name == "a"),
                "expected receiver 'a' at arguments[0], got {:?}",
                &arguments[0]
            );
            assert!(
                matches!(&arguments[1], typed_ast::Expression::Variable { name, .. } if name == "b"),
                "expected user arg 'b' at arguments[1], got {:?}",
                &arguments[1]
            );
        } else {
            panic!("expected Call expression");
        }
    }
    #[test]
    fn string_template_literal_only_has_string_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringTemplate {
                parts: vec![StringPart::Literal("hello".to_string())],
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
    fn string_template_with_interpolated_int_has_string_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::simple("String"),
            vec![Statement::Return(Expression::StringTemplate {
                parts: vec![
                    StringPart::Literal("value is ".to_string()),
                    StringPart::Interpolated(Box::new(Expression::IntLiteral {
                        value: 42,
                        span: crate::types::Span::dummy(),
                    })),
                ],
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
    fn actor_method_call_has_target_set() {
        let counter_src = "fn increment(): String {\n    return \"ok\"\n}\n";
        let main_src = "fn main(): String {\n    let c = spawn<Counter>(\"c1\")\n    return c.increment()\n}\n";
        let parse = |src: &str| {
            parse_program(0)
                .parse(combine::stream::position::Stream::with_positioner(
                    src,
                    combine::stream::position::IndexPositioner::default(),
                ))
                .unwrap()
                .0
        };
        let counter_module = crate::ast::ParsedModule {
            name: nonempty::nonempty!["main".to_string(), "Counter".to_string()],
            module: parse(counter_src),
            is_entry: false,
            file_id: 0,
            is_inline: false,
        };
        let main_module = crate::ast::ParsedModule {
            name: nonempty::NonEmpty::new("main".to_string()),
            module: parse(main_src),
            is_entry: true,
            file_id: 1,
            is_inline: false,
        };
        let mut checker = super::TypeChecker::new();
        let metadata = checker
            .check(&[counter_module, main_module], &native_prelude_modules())
            .expect("typecheck should succeed");
        let main_fn = metadata
            .functions
            .values()
            .find(|f| f.name.last_name() == "main" && f.name.module_prefix().to_string() == "main")
            .unwrap();
        let TypedCheckerAstRef::Function(f, _) = &main_fn.ast_ref else {
            panic!("expected function");
        };
        let return_expr = f
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
            .expect("expected return statement");
        match return_expr {
            typed_ast::Expression::Call {
                target, binding, ..
            } => {
                assert!(target.is_some(), "expected target to be set for actor call");
                assert!(
                    matches!(binding, typed_ast::MethodBinding::Early(_)),
                    "expected Early binding for actor call"
                );
            }
            other => panic!("expected Call expression, got {:?}", other),
        }
    }
}
mod metadata_query_tests {
    use super::*;
    use crate::ast::{SigFunction, StructDefinition, StructField};
    use crate::{CheckerAstRef, TypedCheckerAstRef, TypedRefs};
    use nonempty::NonEmpty;
    use std::sync::Arc;
    use structured_agent_runtime::symbols::{
        DefinitionPath, MetaData, SymbolQuery, TypeDefinitionKind,
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
            .check(&[parsed], &native_prelude_modules())
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
                .module(&DefinitionPath::for_module(NonEmpty::new(
                    "main".to_string()
                )))
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
                .function(&DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("main".to_string())),
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
                .type_def(&DefinitionPath::for_type(
                    DefinitionPath::for_module(NonEmpty::new("main".to_string())),
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
                .type_def(&DefinitionPath::for_type(
                    DefinitionPath::for_module(NonEmpty::new("main".to_string())),
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
            .check(&[parsed], &native_prelude_modules())
            .unwrap();
        let impl_def = metadata.impls.values().find(|v| {
            v.type_name.last_name() == "Foo"
                && v.trait_name.as_ref().map(|t| t.last_name()) == Some("Add")
        });
        assert!(impl_def.is_some());
        assert!(matches!(
            impl_def.unwrap().ast_ref,
            TypedCheckerAstRef::Other(CheckerAstRef::Impl(_))
        ));
    }

    #[test]
    fn test_bare_impl_is_registered() {
        let input = "struct Foo {\n    x: Int,\n}\nimpl Foo {\n    pub fn get(self: Foo): Int {\n        return self.x\n    }\n}\n";
        let module = parse_program(0)
            .parse(combine::stream::position::Stream::with_positioner(
                input,
                combine::stream::position::IndexPositioner::default(),
            ))
            .unwrap()
            .0;
        let metadata = check_meta(module);
        let impl_def = metadata
            .impls
            .values()
            .find(|v| v.type_name.last_name() == "Foo");
        assert!(impl_def.is_some());
        assert!(impl_def.unwrap().trait_name.is_none());
    }

    #[test]
    fn prelude_unit_is_in_symbol_table() {
        let module = create_test_module(vec![]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&DefinitionPath::for_type(
                    DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
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
                .type_def(&DefinitionPath::for_type(
                    DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
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
                .type_def(&DefinitionPath::for_type(
                    DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
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
                AstType::parameterized("List", vec![AstType::simple("Int")]),
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
            .function(&DefinitionPath::for_function(
                DefinitionPath::for_module(NonEmpty::new("main".to_string())),
                "get_items",
            ))
            .unwrap();
        assert_eq!(
            fn_def.type_name,
            DefinitionPath::for_type(
                DefinitionPath::for_module(NonEmpty::new("main".to_string())),
                "get_items",
            )
        );
        let type_def = metadata.type_def(&fn_def.type_name).unwrap();
        if let TypeDefinitionKind::Function { return_type, .. } = &type_def.kind {
            assert_eq!(
                return_type,
                &DefinitionPath::for_type(
                    DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
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
            .function(&DefinitionPath::for_function(
                DefinitionPath::for_module(NonEmpty::new("main".to_string())),
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
            DefinitionPath::for_type(
                DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
                "String",
            )
        );
        assert_eq!(
            return_type,
            &DefinitionPath::for_type(
                DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
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
            .function(&DefinitionPath::for_function(
                DefinitionPath::for_module(NonEmpty::new("main".to_string())),
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
            &DefinitionPath::for_type(
                DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
                "Int",
            )
        );
    }
    #[test]
    fn trait_witness_ref_populated_after_elaboration() {
        use structured_agent_runtime::symbols::WitnessTable;
        let input = concat!(
            "struct Vec2 {\n",
            "    x: Int,\n",
            "}\n",
            "trait Add {\n",
            "    fn add(self: Vec2, other: Vec2): Vec2\n",
            "}\n",
            "impl Vec2: Add {\n",
            "    fn add(self: Vec2, other: Vec2): Vec2 {\n",
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
        let metadata = check_meta(module);
        let main_module = DefinitionPath::for_module(NonEmpty::new("main".to_string()));
        let add_path = DefinitionPath::for_type(main_module.clone(), "Add");
        let vec2_path = DefinitionPath::for_type(main_module.clone(), "Vec2");
        let impl_key = DefinitionPath::for_impl(main_module.clone(), Some(0));
        let trait_def = metadata.type_def(&add_path).expect("Add trait not found");
        let TypeDefinitionKind::Trait { witness_ref, .. } = &trait_def.kind else {
            panic!("expected Trait kind");
        };
        let WitnessTable(map) = witness_ref;
        assert_eq!(
            map.get(&vec2_path),
            Some(&impl_key),
            "witness_ref should map Vec2 -> impl key"
        );
    }
}

#[cfg(test)]
mod constraint_tests {
    use super::*;
    use nonempty::NonEmpty;
    use std::sync::Arc;
    use structured_agent_runtime::Type as RT;

    fn check_constraints(module: crate::ast::Module) -> Vec<crate::solver::Constraint> {
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: true,
            file_id: 0,
            is_inline: false,
        };
        let mut checker = crate::TypeChecker::new();
        checker.get_check_constraints(&[parsed], &native_prelude_modules())
    }

    fn solve_parsed(code: &str) -> crate::solver::SolvedConstraints {
        let stream = combine::stream::position::Stream::with_positioner(
            code,
            combine::stream::position::IndexPositioner::default(),
        );
        let (module, _) = parse_program(0).parse(stream).unwrap();
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: true,
            file_id: 0,
            is_inline: false,
        };
        let mut checker = crate::TypeChecker::new();
        checker.get_solved_constraints(&[parsed], &native_prelude_modules())
    }

    #[test]
    fn unify_constraint_emitted_for_single_type_param() {
        let identity = create_generic_test_function(
            "identity",
            vec!["T".into()],
            vec![create_parameter("x", AstType::simple("T"))],
            AstType::simple("T"),
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType::simple("String"),
            vec![Statement::Return(Expression::Call {
                function: "identity".to_string(),
                type_args: vec![],
                arguments: vec![Expression::Variable {
                    name: "s".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = create_test_module(vec![
            Definition::Function(Arc::new(identity)),
            Definition::Function(Arc::new(caller)),
        ]);
        let constraints = check_constraints(module);
        let unify: Vec<_> = constraints
            .iter()
            .filter(|c| matches!(&c.kind, crate::solver::ConstraintKind::Unify { .. }))
            .collect();
        assert_eq!(unify.len(), 1);
        match &unify[0].kind {
            crate::solver::ConstraintKind::Unify { var, ty, .. } => {
                assert_eq!(var, "T");
                assert!(ty.is_string(), "expected String type, got {:?}", ty);
            }
            _ => panic!("expected Unify"),
        }
    }

    #[test]
    fn unify_constraint_not_emitted_for_monomorphic_call() {
        let func = create_test_function(
            "to_upper",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType::simple("String"),
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType::simple("String"),
            vec![Statement::Return(Expression::Call {
                function: "to_upper".to_string(),
                type_args: vec![],
                arguments: vec![Expression::Variable {
                    name: "s".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = create_test_module(vec![
            Definition::Function(Arc::new(func)),
            Definition::Function(Arc::new(caller)),
        ]);
        let constraints = check_constraints(module);
        let unify_count = constraints
            .iter()
            .filter(|c| matches!(&c.kind, crate::solver::ConstraintKind::Unify { .. }))
            .count();
        assert_eq!(unify_count, 0);
    }

    #[test]
    fn solve_constraints_populates_generic_solutions() {
        let code = "fn identity<T>(x: T): T { return x }\nfn main(s: String): String { return identity(s) }\n";
        let solved = solve_parsed(code);
        assert_eq!(solved.generic_solutions.len(), 1);
        let entry = solved.generic_solutions.values().next().unwrap();
        assert!(
            entry.get("T").map(|t| t.is_string()).unwrap_or(false),
            "expected T -> String, got {:?}",
            entry
        );
    }

    #[test]
    fn solve_constraints_two_call_sites_have_separate_entries() {
        let code = "fn identity<T>(x: T): T { return x }\nfn a(s: String): String { return identity(s) }\nfn b(n: Int): Int { return identity(n) }\n";
        let solved = solve_parsed(code);
        assert_eq!(solved.generic_solutions.len(), 2);
        let types: Vec<&RT> = solved
            .generic_solutions
            .values()
            .filter_map(|m| m.get("T"))
            .collect();
        assert!(
            types.iter().any(|t: &&RT| t.is_string()),
            "missing String entry"
        );
        assert!(types.iter().any(|t: &&RT| t.is_int()), "missing Int entry");
    }

    #[test]
    fn type_mismatch_still_fires_after_unify_change() {
        let func = create_test_function(
            "expects_int",
            vec![create_parameter("n", AstType::simple("Int"))],
            AstType::simple("Int"),
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![create_parameter("s", AstType::simple("String"))],
            AstType::simple("Int"),
            vec![Statement::Return(Expression::Call {
                function: "expects_int".to_string(),
                type_args: vec![],
                arguments: vec![Expression::Variable {
                    name: "s".to_string(),
                    span: crate::types::Span::dummy(),
                }],
                span: crate::types::Span::dummy(),
            })],
        );
        let module = create_test_module(vec![
            Definition::Function(Arc::new(func)),
            Definition::Function(Arc::new(caller)),
        ]);
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: true,
            file_id: 0,
            is_inline: false,
        };
        let result = crate::TypeChecker::new()
            .check(&[parsed], &native_prelude_modules())
            .map(|_| ());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, crate::TypeError::ArgumentTypeMismatch { .. }))
        );
    }
}
