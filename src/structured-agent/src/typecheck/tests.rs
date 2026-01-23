use super::*;
use crate::ast::{
    Definition, Expression, Function, FunctionBody, Module, Parameter, SelectClause,
    SelectExpression, Statement, Type as AstType,
};

fn create_test_module(definitions: Vec<Definition>) -> Module {
    Module { definitions }
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
        body: FunctionBody { statements },
        documentation: None,
    }
}

fn create_parameter(name: &str, param_type: AstType) -> Parameter {
    Parameter {
        name: name.to_string(),
        param_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_function_with_string_parameter() {
        let func = create_test_function(
            "greet",
            vec![create_parameter(
                "name",
                AstType::Named("String".to_string()),
            )],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::Variable("name".to_string()))],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module).is_ok());
    }

    #[test]
    fn test_unknown_variable_error() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Unit,
            vec![Statement::Return(Expression::Variable(
                "unknown".to_string(),
            ))],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TypeError::UnknownVariable(_)));
    }

    #[test]
    fn test_function_call_with_correct_arguments() {
        let greet_func = create_test_function(
            "greet",
            vec![create_parameter(
                "name",
                AstType::Named("String".to_string()),
            )],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::Variable("name".to_string()))],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "greet".to_string(),
                arguments: vec![Expression::StringLiteral("Alice".to_string())],
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(greet_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module).is_ok());
    }

    #[test]
    fn test_function_call_with_wrong_argument_type() {
        let greet_func = create_test_function(
            "greet",
            vec![create_parameter(
                "name",
                AstType::Named("String".to_string()),
            )],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::Variable("name".to_string()))],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "greet".to_string(),
                arguments: vec![Expression::BooleanLiteral(true)],
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(greet_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
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
            vec![create_parameter(
                "name",
                AstType::Named("String".to_string()),
            )],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::Variable("name".to_string()))],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "greet".to_string(),
                arguments: vec![],
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(greet_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::ArgumentCountMismatch { .. }
        ));
    }

    #[test]
    fn test_placeholder_arguments_are_allowed() {
        let func = create_test_function(
            "process",
            vec![create_parameter(
                "data",
                AstType::Named("String".to_string()),
            )],
            AstType::Unit,
            vec![],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "process".to_string(),
                arguments: vec![Expression::Placeholder],
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
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
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::StringLiteral(
                "Alice".to_string(),
            ))],
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
                    },
                },
                Statement::ExpressionStatement(Expression::Variable("name".to_string())),
            ],
        );

        let module = create_test_module(vec![
            Definition::Function(get_name_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
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
                    expression: Expression::BooleanLiteral(true),
                },
                Statement::VariableAssignment {
                    variable: "flag".to_string(),
                    expression: Expression::StringLiteral("hello".to_string()),
                },
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::TypeMismatch { .. }
        ));
    }

    #[test]
    fn test_if_condition_must_be_boolean() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Unit,
            vec![Statement::If {
                condition: Expression::StringLiteral("hello".to_string()),
                body: vec![],
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
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
                condition: Expression::StringLiteral("hello".to_string()),
                body: vec![],
            }],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
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
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::BooleanLiteral(true))],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
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
                create_parameter("a", AstType::Named("String".to_string())),
                create_parameter("b", AstType::Named("String".to_string())),
            ],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::StringLiteral(
                "result".to_string(),
            ))],
        );

        let concat_func = create_test_function(
            "concat",
            vec![
                create_parameter("x", AstType::Named("String".to_string())),
                create_parameter("y", AstType::Named("String".to_string())),
            ],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::StringLiteral(
                "concatenated".to_string(),
            ))],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::Select(SelectExpression {
                clauses: vec![
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "add".to_string(),
                            arguments: vec![Expression::Placeholder, Expression::Placeholder],
                        },
                        result_variable: "sum".to_string(),
                        expression_next: Expression::Variable("sum".to_string()),
                    },
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "concat".to_string(),
                            arguments: vec![Expression::Placeholder, Expression::Placeholder],
                        },
                        result_variable: "text".to_string(),
                        expression_next: Expression::Variable("text".to_string()),
                    },
                ],
            }))],
        );

        let module = create_test_module(vec![
            Definition::Function(add_func),
            Definition::Function(concat_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module).is_ok());
    }

    #[test]
    fn test_select_branch_type_mismatch() {
        let get_string_func = create_test_function(
            "get_string",
            vec![],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::StringLiteral(
                "text".to_string(),
            ))],
        );

        let get_bool_func = create_test_function(
            "get_bool",
            vec![],
            AstType::Boolean,
            vec![Statement::Return(Expression::BooleanLiteral(true))],
        );

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Named("String".to_string()),
            vec![Statement::Return(Expression::Select(SelectExpression {
                clauses: vec![
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "get_string".to_string(),
                            arguments: vec![],
                        },
                        result_variable: "str_result".to_string(),
                        expression_next: Expression::Variable("str_result".to_string()),
                    },
                    SelectClause {
                        expression_to_run: Expression::Call {
                            function: "get_bool".to_string(),
                            arguments: vec![],
                        },
                        result_variable: "bool_result".to_string(),
                        expression_next: Expression::Variable("bool_result".to_string()),
                    },
                ],
            }))],
        );

        let module = create_test_module(vec![
            Definition::Function(get_string_func),
            Definition::Function(get_bool_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeError::SelectBranchTypeMismatch { .. }
        ));
    }

    #[test]
    fn test_unsupported_type_error() {
        let func = create_test_function(
            "test",
            vec![create_parameter(
                "x",
                AstType::Named("CustomType".to_string()),
            )],
            AstType::Unit,
            vec![],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TypeError::UnsupportedType(_)));
    }

    #[test]
    fn test_context_type_is_supported() {
        let func = create_test_function(
            "test",
            vec![create_parameter(
                "ctx",
                AstType::Named("Context".to_string()),
            )],
            AstType::Unit,
            vec![],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module).is_ok());
    }

    #[test]
    fn test_external_function_type_checking() {
        use crate::ast::ExternalFunction;

        let ext_func = ExternalFunction {
            name: "external_add".to_string(),
            parameters: vec![
                create_parameter("a", AstType::Named("String".to_string())),
                create_parameter("b", AstType::Named("String".to_string())),
            ],
            return_type: AstType::Named("String".to_string()),
        };

        let main_func = create_test_function(
            "main",
            vec![],
            AstType::Unit,
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "external_add".to_string(),
                arguments: vec![
                    Expression::StringLiteral("hello".to_string()),
                    Expression::StringLiteral("world".to_string()),
                ],
            })],
        );

        let module = create_test_module(vec![
            Definition::ExternalFunction(ext_func),
            Definition::Function(main_func),
        ]);
        let mut checker = TypeChecker::new();

        assert!(checker.check_module(&module).is_ok());
    }

    #[test]
    fn test_nested_scope_variable_isolation() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Unit,
            vec![
                Statement::If {
                    condition: Expression::BooleanLiteral(true),
                    body: vec![Statement::Assignment {
                        variable: "inner_var".to_string(),
                        expression: Expression::StringLiteral("hello".to_string()),
                    }],
                },
                Statement::ExpressionStatement(Expression::Variable("inner_var".to_string())),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TypeError::UnknownVariable(_)));
    }

    #[test]
    fn test_variable_shadowing_should_not_leak() {
        let func = create_test_function(
            "test",
            vec![],
            AstType::Named("String".to_string()),
            vec![
                Statement::Assignment {
                    variable: "result".to_string(),
                    expression: Expression::StringLiteral("foo".to_string()),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral(true),
                    body: vec![Statement::Assignment {
                        variable: "result".to_string(),
                        expression: Expression::BooleanLiteral(true), // Different type - should shadow locally
                    }],
                },
                // After if block, result should still be String type from outer scope
                Statement::Return(Expression::Variable("result".to_string())),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        let result = checker.check_module(&module);
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
            AstType::Named("String".to_string()),
            vec![
                Statement::Assignment {
                    variable: "x".to_string(),
                    expression: Expression::StringLiteral("outer".to_string()),
                },
                Statement::If {
                    condition: Expression::BooleanLiteral(true),
                    body: vec![
                        Statement::Assignment {
                            variable: "y".to_string(),
                            expression: Expression::StringLiteral("middle".to_string()),
                        },
                        Statement::If {
                            condition: Expression::BooleanLiteral(true),
                            body: vec![
                                Statement::Assignment {
                                    variable: "z".to_string(),
                                    expression: Expression::StringLiteral("inner".to_string()),
                                },
                                // Should be able to access x from outer scope
                                Statement::ExpressionStatement(Expression::Variable(
                                    "x".to_string(),
                                )),
                                // Should be able to access y from middle scope
                                Statement::ExpressionStatement(Expression::Variable(
                                    "y".to_string(),
                                )),
                            ],
                        },
                        // z should not be accessible here
                    ],
                },
                Statement::Return(Expression::Variable("x".to_string())),
            ],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut checker = TypeChecker::new();

        // This should pass with proper scope chaining
        let result = checker.check_module(&module);
        if let Err(ref e) = result {
            println!("Error: {}", e);
        }
        assert!(result.is_ok());
    }
}
