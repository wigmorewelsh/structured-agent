#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, UnusedReturnValueAnalyzer, Warning};
    use crate::ast::{
        Definition, Expression, ExternalFunction, Function, FunctionBody, Module, Parameter,
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

    fn create_external_function(name: &str, return_type: Type) -> ExternalFunction {
        ExternalFunction {
            name: name.to_string(),
            parameters: vec![],
            return_type,
            span: Span::dummy(),
        }
    }

    #[test]
    fn detects_unused_return_value_from_external_function() {
        let external_func = create_external_function("get_data", Type::Named("String".to_string()));

        let func = create_test_function(
            "test",
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "get_data".to_string(),
                arguments: vec![],
                span: Span::new(0, 10),
            })],
        );

        let module = create_test_module(vec![
            Definition::ExternalFunction(external_func),
            Definition::Function(func),
        ]);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        if let Warning::UnusedReturnValue { function_name, .. } = &warnings[0] {
            assert_eq!(function_name, "get_data");
        } else {
            panic!("Expected UnusedReturnValue warning");
        }
    }

    #[test]
    fn no_warning_when_return_value_is_used() {
        let external_func = create_external_function("get_data", Type::Named("String".to_string()));

        let func = create_test_function(
            "test",
            vec![Statement::Assignment {
                variable: "result".to_string(),
                expression: Expression::Call {
                    function: "get_data".to_string(),
                    arguments: vec![],
                    span: Span::dummy(),
                },
                span: Span::new(0, 10),
            }],
        );

        let module = create_test_module(vec![
            Definition::ExternalFunction(external_func),
            Definition::Function(func),
        ]);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn no_warning_for_unit_return_type() {
        let external_func = create_external_function("log", Type::Unit);

        let func = create_test_function(
            "test",
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "log".to_string(),
                arguments: vec![],
                span: Span::new(0, 10),
            })],
        );

        let module = create_test_module(vec![
            Definition::ExternalFunction(external_func),
            Definition::Function(func),
        ]);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_unused_return_from_internal_function() {
        let returning_func = Function {
            name: "get_value".to_string(),
            parameters: vec![],
            return_type: Type::Named("String".to_string()),
            body: FunctionBody {
                statements: vec![Statement::Return(Expression::StringLiteral {
                    value: "result".to_string(),
                    span: Span::dummy(),
                })],
                span: Span::dummy(),
            },
            documentation: None,
            span: Span::dummy(),
        };

        let func = create_test_function(
            "test",
            vec![Statement::ExpressionStatement(Expression::Call {
                function: "get_value".to_string(),
                arguments: vec![],
                span: Span::new(0, 10),
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(returning_func),
            Definition::Function(func),
        ]);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        if let Warning::UnusedReturnValue { function_name, .. } = &warnings[0] {
            assert_eq!(function_name, "get_value");
        } else {
            panic!("Expected UnusedReturnValue warning");
        }
    }

    #[test]
    fn no_warning_when_used_in_injection() {
        let external_func = create_external_function("get_data", Type::Named("String".to_string()));

        let func = create_test_function(
            "test",
            vec![Statement::Injection(Expression::Call {
                function: "get_data".to_string(),
                arguments: vec![],
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::ExternalFunction(external_func),
            Definition::Function(func),
        ]);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_unused_returns() {
        let external_func1 =
            create_external_function("get_data1", Type::Named("String".to_string()));
        let external_func2 =
            create_external_function("get_data2", Type::Named("String".to_string()));

        let func = create_test_function(
            "test",
            vec![
                Statement::ExpressionStatement(Expression::Call {
                    function: "get_data1".to_string(),
                    arguments: vec![],
                    span: Span::new(0, 10),
                }),
                Statement::ExpressionStatement(Expression::Call {
                    function: "get_data2".to_string(),
                    arguments: vec![],
                    span: Span::new(20, 30),
                }),
            ],
        );

        let module = create_test_module(vec![
            Definition::ExternalFunction(external_func1),
            Definition::ExternalFunction(external_func2),
            Definition::Function(func),
        ]);
        let mut analyzer = UnusedReturnValueAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }
}
