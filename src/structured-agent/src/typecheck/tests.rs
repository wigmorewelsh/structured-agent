use super::*;
use crate::ast::{
    AstTrait, Definition, Expression, Function, FunctionBody, Module, Parameter, SelectClause,
    SelectExpression, Statement, Type as AstType,
};
use crate::compiler::parser::parse_program;
use combine::{Parser, Stream, stream::position::IndexPositioner};

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
    use std::sync::Arc;

    fn check(module: crate::ast::Module) -> Result<(), crate::typecheck::TypeError> {
        let parsed = crate::ast::ParsedModule {
            name: "test".to_string(),
            module,
            is_entry: true,
            file_id: 0,
        };
        TypeChecker::new()
            .check_modules(&[parsed], &std::collections::HashMap::new())
            .map(|_| ())
    }

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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        assert!(check(module).is_ok());
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
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
            Definition::Function(Arc::new(greet_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        assert!(check(module).is_ok());
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
            Definition::Function(Arc::new(greet_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
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
            Definition::Function(Arc::new(greet_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
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
            Definition::Function(Arc::new(test_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
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
            Definition::Function(Arc::new(get_name_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
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
            Definition::Function(Arc::new(get_string_func)),
            Definition::Function(Arc::new(get_bool_func)),
            Definition::Function(Arc::new(main_func)),
        ]);

        let result = check(module);
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
            type_params: vec![],
            parameters: vec![
                create_parameter("value1", AstType::String),
                create_parameter("id", AstType::String),
            ],
            return_type: AstType::String,
            is_pub: false,
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
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

        let module = create_test_module(vec![Definition::Function(Arc::new(func))]);

        let result = check(module);
        if let Err(ref e) = result {
            println!("Error: {}", e);
        }
        assert!(result.is_ok());
    }

    fn create_struct_definition(name: &str, fields: Vec<(&str, AstType)>) -> Definition {
        use crate::ast::{StructDefinition, StructField};
        Definition::Struct(Arc::new(StructDefinition {
            name: name.to_string(),
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
            create_struct_definition("Point", vec![("x", AstType::Int), ("y", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "main",
                vec![],
                AstType::Unit,
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
            create_struct_definition("Task", vec![("title", AstType::String)]),
            Definition::Function(Arc::new(create_test_function(
                "get_title",
                vec![create_parameter("t", AstType::Struct("Task".to_string()))],
                AstType::String,
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
                vec![create_parameter(
                    "x",
                    AstType::Struct("Unknown".to_string()),
                )],
                AstType::Unit,
                vec![Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let result = check(module);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::UnsupportedType { .. }
        ));
    }

    #[test]
    fn test_struct_literal_valid() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::Int), ("y", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::Struct("Point".to_string()),
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
                AstType::Unit,
                vec![Statement::Return(Expression::StructLiteral {
                    struct_name: "Ghost".to_string(),
                    fields: vec![],
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let result = check(module);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::UnsupportedType { .. }
        ));
    }

    #[test]
    fn test_struct_literal_unknown_field_is_error() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::Unit,
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
        assert!(matches!(
            result.unwrap_err(),
            TypeError::UnknownField { .. }
        ));
    }

    #[test]
    fn test_struct_literal_field_type_mismatch_is_error() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::Unit,
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
        assert!(matches!(
            result.unwrap_err(),
            TypeError::StructFieldTypeMismatch { .. }
        ));
    }

    #[test]
    fn test_struct_literal_missing_field_span_does_not_bleed() {
        let source = "struct Point {\n    x: Int,\n    y: Int,\n}\n\nfn main(): Int {\n    let p = Point { x: 1 }\n    return p.x\n}\n";
        let stream =
            combine::stream::position::Stream::with_positioner(source, IndexPositioner::default());
        let (module, _) = parse_program(0).parse(stream).unwrap();
        let err = check(module).unwrap_err();
        let TypeError::MissingField { span, .. } = err else {
            panic!("Expected MissingField, got {:?}", err);
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
            create_struct_definition("Point", vec![("x", AstType::Int), ("y", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::Struct("Point".to_string()),
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
        assert!(matches!(
            result.unwrap_err(),
            TypeError::MissingField { field_name, .. } if field_name == "y"
        ));
    }

    #[test]
    fn test_field_access_valid() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::Int), ("y", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "get_x",
                vec![create_parameter("p", AstType::Struct("Point".to_string()))],
                AstType::Int,
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
            create_struct_definition("Point", vec![("x", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "get_z",
                vec![create_parameter("p", AstType::Struct("Point".to_string()))],
                AstType::Int,
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
        assert!(matches!(
            result.unwrap_err(),
            TypeError::UnknownField { .. }
        ));
    }

    #[test]
    fn test_field_access_on_non_struct_is_error() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "bad",
                vec![create_parameter("s", AstType::String)],
                AstType::Int,
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
        assert!(matches!(
            result.unwrap_err(),
            TypeError::TypeMismatch { .. }
        ));
    }

    #[test]
    fn test_struct_defined_after_function_that_uses_it_is_valid() {
        let module = create_test_module(vec![
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::Struct("Point".to_string()),
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
            create_struct_definition("Point", vec![("x", AstType::Int)]),
        ]);
        assert!(check(module).is_ok());
    }

    #[test]
    fn test_struct_literal_as_call_argument_is_valid() {
        let module = create_test_module(vec![
            create_struct_definition("Point", vec![("x", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "consume",
                vec![create_parameter("p", AstType::Struct("Point".to_string()))],
                AstType::Unit,
                vec![Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                })],
            ))),
            Definition::Function(Arc::new(create_test_function(
                "make_and_pass",
                vec![],
                AstType::Unit,
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
            create_struct_definition("Point", vec![("x", AstType::Int)]),
            Definition::ExternalFunction(Arc::new(crate::ast::ExternalFunction {
                name: "get_point".to_string(),
                type_params: vec![],
                parameters: vec![],
                return_type: AstType::Struct("Point".to_string()),
                is_pub: false,
                span: crate::types::Span::dummy(),
            })),
            Definition::Function(Arc::new(create_test_function(
                "main",
                vec![],
                AstType::Unit,
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
            create_struct_definition("Point", vec![("x", AstType::Int), ("y", AstType::Int)]),
            Definition::Function(Arc::new(create_test_function(
                "make",
                vec![],
                AstType::Struct("Point".to_string()),
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
        assert!(matches!(
            result.unwrap_err(),
            TypeError::DuplicateField { field_name, .. } if field_name == "x"
        ));
    }

    #[test]
    fn test_extern_fn_with_unknown_struct_return_type_is_error() {
        let module = create_test_module(vec![Definition::ExternalFunction(Arc::new(
            crate::ast::ExternalFunction {
                name: "get_ghost".to_string(),
                type_params: vec![],
                parameters: vec![],
                return_type: AstType::Struct("Ghost".to_string()),
                is_pub: false,
                span: crate::types::Span::dummy(),
            },
        ))]);
        let result = check(module);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::UnsupportedType { .. }
        ));
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
                AstType::List(Box::new(AstType::Generic("T".to_string()))),
            )],
            AstType::Option(Box::new(AstType::Generic("T".to_string()))),
            vec![],
        );
        let caller = create_test_function(
            "caller",
            vec![create_parameter("s", AstType::String)],
            AstType::Option(Box::new(AstType::String)),
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
        assert!(
            matches!(result, Err(TypeError::ArgumentTypeMismatch { ref expected, ref found, .. })
                if expected == "List<T>" && found == "String"),
            "expected ArgumentTypeMismatch with concrete type names, got {:?}",
            result
        );
    }

    #[test]
    fn test_generic_extern_fn_with_type_params_type_checks() {
        let ext_func = crate::ast::ExternalFunction {
            name: "wrap".to_string(),
            type_params: vec!["T".into()],
            parameters: vec![create_parameter("value", AstType::Generic("T".to_string()))],
            return_type: AstType::Option(Box::new(AstType::Generic("T".to_string()))),
            is_pub: false,
            span: crate::types::Span::dummy(),
        };
        let caller = create_test_function(
            "main",
            vec![create_parameter("s", AstType::String)],
            AstType::Option(Box::new(AstType::String)),
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
            AstType::Option(Box::new(AstType::Generic("T".to_string()))),
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![],
            AstType::Unit,
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
                AstType::List(Box::new(AstType::Generic("T".to_string()))),
            )],
            AstType::Option(Box::new(AstType::Generic("T".to_string()))),
            vec![],
        );
        let consume = create_test_function(
            "consume",
            vec![create_parameter(
                "item",
                AstType::Option(Box::new(AstType::String)),
            )],
            AstType::Unit,
            vec![],
        );
        let caller = create_test_function(
            "main",
            vec![create_parameter(
                "xs",
                AstType::List(Box::new(AstType::String)),
            )],
            AstType::Unit,
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
            vec![create_parameter("x", AstType::Generic("T".to_string()))],
            AstType::Generic("T".to_string()),
            vec![],
        );
        let module = create_test_module(vec![Definition::Function(Arc::new(bad))]);
        let result = check(module);
        assert!(
            matches!(result, Err(TypeError::UnsupportedType { ref type_name, .. }) if type_name == "T"),
            "expected UnsupportedType for unknown type variable, got {:?}",
            result
        );
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
        assert!(
            matches!(result, Err(TypeError::TraitBoundNotSatisfied { ref type_name, ref trait_name, .. })
                if type_name == "String" && trait_name == "Add"),
            "String does not satisfy Add, expected TraitBoundNotSatisfied, got {:?}",
            result
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
        let result = check(module);
        assert!(
            matches!(result, Err(TypeError::TraitImplMissingFunction { ref function_name, .. })
                if function_name == "add"),
            "impl missing 'add' should be error, got {:?}",
            result
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
        let result = check(module);
        assert!(
            matches!(result, Err(TypeError::UnknownTrait { ref name, .. }) if name == "NonExistent"),
            "impl for unknown trait should error, got {:?}",
            result
        );
    }
}

#[cfg(test)]
mod typed_ast_tests {
    use super::*;
    use crate::typecheck::checker::{FunctionKind, TypedCheckerAstRef};
    use crate::typed_ast;
    use std::collections::HashMap;
    use std::sync::Arc;
    use structured_agent_runtime::symbols::{
        FunctionName, FunctionNameKind, ModuleName, TraitName, TypeName,
    };

    fn check_typed(module: &Module) -> typed_ast::Module {
        let parsed = crate::ast::ParsedModule {
            name: "main".to_string(),
            module: module.clone(),
            is_entry: false,
            file_id: 0,
        };
        let typed_metadata = TypeChecker::new()
            .check_modules(&[parsed], &std::collections::HashMap::new())
            .unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module.to_string() != "main" {
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

    fn first_stmt(module: &typed_ast::Module) -> &typed_ast::Statement {
        first_function(module).body.statements.first().unwrap()
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
            AstType::String,
            vec![Statement::Return(Expression::StringLiteral {
                value: "hello".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &AstType::String);
    }

    #[test]
    fn boolean_literal_has_boolean_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::Boolean,
            vec![Statement::Return(Expression::BooleanLiteral {
                value: true,
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &AstType::Boolean);
    }

    #[test]
    fn int_literal_has_int_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::Int,
            vec![Statement::Return(Expression::IntLiteral {
                value: 42,
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &AstType::Int);
    }

    #[test]
    fn unit_literal_has_unit_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::Unit,
            vec![Statement::Return(Expression::UnitLiteral {
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &AstType::Unit);
    }

    #[test]
    fn variable_expression_carries_declared_type() {
        let func = create_test_function(
            "f",
            vec![create_parameter("x", AstType::Int)],
            AstType::Int,
            vec![Statement::Return(Expression::Variable {
                name: "x".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let module = check_typed(&create_test_module(vec![Definition::Function(Arc::new(
            func,
        ))]));
        let expr = stmt_expr(first_function(&module).body.statements.first().unwrap());
        assert_eq!(expr.ty(), &AstType::Int);
        assert!(matches!(expr, typed_ast::Expression::Variable { name, .. } if name == "x"));
    }

    #[test]
    fn call_carries_return_type_and_resolved_name() {
        let callee = create_test_function(
            "get_value",
            vec![],
            AstType::String,
            vec![Statement::Return(Expression::StringLiteral {
                value: "v".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let caller = create_test_function(
            "f",
            vec![],
            AstType::String,
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
        assert_eq!(expr.ty(), &AstType::String);
        assert!(matches!(
            expr,
            typed_ast::Expression::Call { resolved, kind: FunctionKind::Bytecode, .. }
            if resolved == &FunctionName { name: "get_value".to_string(), module: ModuleName::from_str("main"), kind: FunctionNameKind::Function }
        ));
    }

    #[test]
    fn placeholder_in_call_carries_parameter_type() {
        let callee = create_test_function(
            "process",
            vec![create_parameter("s", AstType::String)],
            AstType::Unit,
            vec![Statement::Return(Expression::UnitLiteral {
                span: crate::types::Span::dummy(),
            })],
        );
        let caller = create_test_function(
            "f",
            vec![],
            AstType::Unit,
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
            assert_eq!(arguments[0].ty(), &AstType::String);
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
            AstType::Unit,
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
            assert_eq!(expression.ty(), &AstType::Int);
        } else {
            panic!("expected Assignment");
        }
    }

    #[test]
    fn list_literal_has_list_type() {
        let func = create_test_function(
            "f",
            vec![],
            AstType::List(Box::new(AstType::Int)),
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
        assert_eq!(expr.ty(), &AstType::List(Box::new(AstType::Int)));
    }

    #[test]
    fn if_else_expression_has_branch_type() {
        let func = create_test_function(
            "f",
            vec![create_parameter("flag", AstType::Boolean)],
            AstType::Int,
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
        assert_eq!(expr.ty(), &AstType::Int);
        assert!(matches!(expr, typed_ast::Expression::IfElse { .. }));
    }

    #[test]
    fn if_statement_wraps_typed_condition_and_body() {
        let func = create_test_function(
            "f",
            vec![create_parameter("flag", AstType::Boolean)],
            AstType::Unit,
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
            assert_eq!(condition.ty(), &AstType::Boolean);
            assert!(!body.is_empty());
        } else {
            panic!("expected If");
        }
    }

    #[test]
    fn struct_literal_has_struct_type() {
        let struct_def = crate::ast::StructDefinition {
            name: "Point".to_string(),
            fields: vec![
                crate::ast::StructField {
                    name: "x".to_string(),
                    field_type: AstType::Int,
                    span: crate::types::Span::dummy(),
                },
                crate::ast::StructField {
                    name: "y".to_string(),
                    field_type: AstType::Int,
                    span: crate::types::Span::dummy(),
                },
            ],
            span: crate::types::Span::dummy(),
        };
        let func = create_test_function(
            "f",
            vec![],
            AstType::Struct("Point".to_string()),
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
        assert_eq!(expr.ty(), &AstType::Struct("Point".to_string()));
    }

    #[test]
    fn field_access_has_field_type() {
        let struct_def = crate::ast::StructDefinition {
            name: "Point".to_string(),
            fields: vec![
                crate::ast::StructField {
                    name: "x".to_string(),
                    field_type: AstType::Int,
                    span: crate::types::Span::dummy(),
                },
                crate::ast::StructField {
                    name: "y".to_string(),
                    field_type: AstType::Int,
                    span: crate::types::Span::dummy(),
                },
            ],
            span: crate::types::Span::dummy(),
        };
        let func = create_test_function(
            "f",
            vec![create_parameter("p", AstType::Struct("Point".to_string()))],
            AstType::Int,
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
        assert_eq!(expr.ty(), &AstType::Int);
    }

    #[test]
    fn select_expression_has_branch_type() {
        let callee = create_test_function(
            "get_str",
            vec![],
            AstType::String,
            vec![Statement::Return(Expression::StringLiteral {
                value: "v".to_string(),
                span: crate::types::Span::dummy(),
            })],
        );
        let func = create_test_function(
            "f",
            vec![],
            AstType::String,
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
        assert_eq!(expr.ty(), &AstType::String);
        assert!(matches!(expr, typed_ast::Expression::Select(_, _)));
    }

    #[test]
    fn generic_head_with_string_list_returns_option_string() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType::List(Box::new(AstType::Generic("T".to_string()))),
            )],
            AstType::Option(Box::new(AstType::Generic("T".to_string()))),
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![create_parameter(
                "xs",
                AstType::List(Box::new(AstType::String)),
            )],
            AstType::Option(Box::new(AstType::String)),
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
        assert_eq!(expr.ty(), &AstType::Option(Box::new(AstType::String)));
    }

    #[test]
    fn generic_head_with_int_list_returns_option_int() {
        let head = create_generic_test_function(
            "head",
            vec!["T".into()],
            vec![create_parameter(
                "list",
                AstType::List(Box::new(AstType::Generic("T".to_string()))),
            )],
            AstType::Option(Box::new(AstType::Generic("T".to_string()))),
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![create_parameter(
                "xs",
                AstType::List(Box::new(AstType::Int)),
            )],
            AstType::Option(Box::new(AstType::Int)),
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
        assert_eq!(expr.ty(), &AstType::Option(Box::new(AstType::Int)));
    }

    #[test]
    fn generic_zip_with_two_type_params_typechecks() {
        let zip = create_generic_test_function(
            "zip",
            vec!["A".into(), "B".into()],
            vec![
                create_parameter(
                    "a",
                    AstType::List(Box::new(AstType::Generic("A".to_string()))),
                ),
                create_parameter(
                    "b",
                    AstType::List(Box::new(AstType::Generic("B".to_string()))),
                ),
            ],
            AstType::List(Box::new(AstType::Generic("A".to_string()))),
            vec![],
        );
        let caller = create_test_function(
            "f",
            vec![
                create_parameter("strs", AstType::List(Box::new(AstType::String))),
                create_parameter("ints", AstType::List(Box::new(AstType::Int))),
            ],
            AstType::List(Box::new(AstType::String)),
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
        assert_eq!(expr.ty(), &AstType::List(Box::new(AstType::String)));
    }

    #[test]
    fn external_call_carries_external_kind() {
        let ext = crate::ast::ExternalFunction {
            name: "native_fn".to_string(),
            type_params: vec![],
            parameters: vec![],
            return_type: AstType::Int,
            is_pub: false,
            span: crate::types::Span::dummy(),
        };
        let caller = create_test_function(
            "f",
            vec![],
            AstType::Int,
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
            let mn = ModuleName::from_str("main");
            FunctionName {
                name: "add".to_string(),
                module: mn.clone(),
                kind: FunctionNameKind::Impl {
                    type_name: TypeName {
                        name: "Vec2".to_string(),
                        module: mn.clone(),
                    },
                    trait_name: TraitName {
                        name: "Add".to_string(),
                        module: mn,
                    },
                },
            }
        });
    }
}

mod metadata_query_tests {
    use super::*;
    use crate::ast::{SigFunction, StructDefinition, StructField};
    use crate::typecheck::checker::{CheckerAstRef, TypedCheckerAstRef, TypedRefs};
    use std::sync::Arc;
    use structured_agent_runtime::symbols::{
        FunctionName, FunctionNameKind, MetaData, ModuleName, SymbolQuery, TraitName, TypeName,
    };

    fn check_meta(module: crate::ast::Module) -> MetaData<TypedRefs> {
        let parsed = crate::ast::ParsedModule {
            name: "main".to_string(),
            module,
            is_entry: true,
            file_id: 0,
        };
        let metadata = TypeChecker::new()
            .check_modules(&[parsed], &std::collections::HashMap::new())
            .unwrap();
        metadata
    }

    #[test]
    fn metadata_module_is_populated() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "main",
                vec![],
                AstType::Unit,
                vec![Statement::Return(Expression::UnitLiteral {
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let metadata = check_meta(module);
        assert!(metadata.module(&ModuleName::from_str("main")).is_some());
    }

    #[test]
    fn metadata_function_is_queryable() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "greet",
                vec![create_parameter("name", AstType::String)],
                AstType::String,
                vec![Statement::Return(Expression::Variable {
                    name: "name".to_string(),
                    span: crate::types::Span::dummy(),
                })],
            )))]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .function(&FunctionName {
                    name: "greet".to_string(),
                    module: ModuleName::from_str("main"),
                    kind: FunctionNameKind::Function,
                })
                .is_some()
        );
    }

    #[test]
    fn metadata_struct_type_is_queryable() {
        let module = create_test_module(vec![Definition::Struct(Arc::new(StructDefinition {
            name: "Point".to_string(),
            fields: vec![StructField {
                name: "x".to_string(),
                field_type: AstType::Int,
                span: crate::types::Span::dummy(),
            }],
            span: crate::types::Span::dummy(),
        }))]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&TypeName {
                    name: "Point".to_string(),
                    module: ModuleName::from_str("main"),
                })
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
                return_type: AstType::String,
                span: crate::types::Span::dummy(),
            }],
            span: crate::types::Span::dummy(),
        }))]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .trait_def(&TraitName {
                    name: "Greetable".to_string(),
                    module: ModuleName::from_str("main"),
                })
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
            name: "main".to_string(),
            module,
            is_entry: true,
            file_id: 0,
        };
        let metadata = TypeChecker::new()
            .check_modules(&[parsed], &std::collections::HashMap::new())
            .unwrap();
        let type_name = TypeName {
            name: "Foo".to_string(),
            module: ModuleName::from_str("main"),
        };
        let trait_name = TraitName {
            name: "Add".to_string(),
            module: ModuleName::from_str("main"),
        };
        let impl_def = metadata.impl_for(&type_name, &trait_name);
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
                .type_def(&TypeName {
                    name: "()".to_string(),
                    module: ModuleName::from_str("prelude"),
                })
                .is_some()
        );
    }

    #[test]
    fn prelude_string_is_in_symbol_table() {
        let module = create_test_module(vec![]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&TypeName {
                    name: "String".to_string(),
                    module: ModuleName::from_str("prelude"),
                })
                .is_some()
        );
    }

    #[test]
    fn prelude_list_is_in_symbol_table() {
        let module = create_test_module(vec![]);
        let metadata = check_meta(module);
        assert!(
            metadata
                .type_def(&TypeName {
                    name: "List".to_string(),
                    module: ModuleName::from_str("prelude"),
                })
                .is_some()
        );
    }

    #[test]
    fn list_return_type_resolves_to_prelude_list() {
        let module =
            create_test_module(vec![Definition::Function(Arc::new(create_test_function(
                "get_items",
                vec![],
                AstType::List(Box::new(AstType::Int)),
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
            .function(&FunctionName {
                name: "get_items".to_string(),
                module: ModuleName::from_str("main"),
                kind: FunctionNameKind::Function,
            })
            .unwrap();
        assert_eq!(
            fn_def.type_name,
            TypeName {
                name: "List".to_string(),
                module: ModuleName::from_str("prelude"),
            }
        );
    }
}
