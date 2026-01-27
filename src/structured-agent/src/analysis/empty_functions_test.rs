#[cfg(test)]
mod tests {
    use crate::analysis::{Analyzer, EmptyFunctionAnalyzer, Warning};
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
    fn detects_empty_function() {
        let func = create_test_function("empty", vec![]);

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = EmptyFunctionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::EmptyFunction { .. }));
    }

    #[test]
    fn no_warning_for_function_with_statements() {
        let func = create_test_function(
            "not_empty",
            vec![Statement::Injection(Expression::StringLiteral {
                value: "has content".to_string(),
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = EmptyFunctionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn detects_multiple_empty_functions() {
        let func1 = create_test_function("empty1", vec![]);
        let func2 = create_test_function("empty2", vec![]);
        let func3 = create_test_function(
            "not_empty",
            vec![Statement::Injection(Expression::StringLiteral {
                value: "content".to_string(),
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![
            Definition::Function(func1),
            Definition::Function(func2),
            Definition::Function(func3),
        ]);
        let mut analyzer = EmptyFunctionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_function_with_return() {
        let func = create_test_function(
            "returns_value",
            vec![Statement::Return(Expression::StringLiteral {
                value: "result".to_string(),
                span: Span::dummy(),
            })],
        );

        let module = create_test_module(vec![Definition::Function(func)]);
        let mut analyzer = EmptyFunctionAnalyzer::new();
        let warnings = analyzer.analyze_module(&module, 0);

        assert_eq!(warnings.len(), 0);
    }
}
