use crate::ast;
use crate::expressions::{
    AssignmentExpr, CallExpr, FunctionExpr, InjectionExpr, StringLiteralExpr, VariableExpr,
};
use crate::types::{Expression, Type};

fn convert_ast_type_to_type(ast_type: &ast::Type) -> Type {
    match ast_type {
        ast::Type::Named(name) => Type { name: name.clone() },
        ast::Type::Unit => Type::unit(),
    }
}

pub struct Compiler;

impl Compiler {
    pub fn compile_expression(ast_expr: &ast::Expression) -> Result<Box<dyn Expression>, String> {
        match ast_expr {
            ast::Expression::StringLiteral(value) => Ok(Box::new(StringLiteralExpr {
                value: value.clone(),
            })),
            ast::Expression::Variable(name) => Ok(Box::new(VariableExpr { name: name.clone() })),
            ast::Expression::Call {
                target,
                function,
                arguments,
                is_method,
            } => {
                let compiled_args = arguments
                    .iter()
                    .map(|arg| Self::compile_expression(arg))
                    .collect::<Result<Vec<_>, String>>()?;

                Ok(Box::new(CallExpr {
                    target: target.clone(),
                    function: function.clone(),
                    arguments: compiled_args,
                    is_method: *is_method,
                }))
            }
        }
    }

    pub fn compile_statement(ast_stmt: &ast::Statement) -> Result<Box<dyn Expression>, String> {
        match ast_stmt {
            ast::Statement::Injection(expr) => {
                let compiled_inner = Self::compile_expression(expr)?;
                Ok(Box::new(InjectionExpr {
                    inner: compiled_inner,
                }))
            }
            ast::Statement::Assignment {
                variable,
                expression,
            } => {
                let compiled_expression = Self::compile_expression(expression)?;
                Ok(Box::new(AssignmentExpr {
                    variable: variable.clone(),
                    expression: compiled_expression,
                }))
            }
        }
    }

    pub fn compile_function(ast_func: &ast::Function) -> Result<FunctionExpr, String> {
        let mut compiled_statements = Vec::new();

        for stmt in &ast_func.body.statements {
            let compiled_stmt = Self::compile_statement(stmt)?;
            compiled_statements.push(compiled_stmt);
        }

        Ok(FunctionExpr {
            name: ast_func.name.clone(),
            parameters: ast_func
                .parameters
                .iter()
                .map(|p| (p.name.clone(), convert_ast_type_to_type(&p.param_type)))
                .collect(),
            return_type: convert_ast_type_to_type(&ast_func.return_type),
            body: compiled_statements,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expression as AstExpression, Statement as AstStatement};
    use crate::types::{Context, ExprResult};

    #[test]
    fn test_compile_string_literal() {
        let ast_expr = AstExpression::StringLiteral("Hello".to_string());
        let compiled = Compiler::compile_expression(&ast_expr).unwrap();

        let mut context = Context::new();
        let result = compiled.evaluate(&mut context).unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("Expected string result"),
        }
    }

    #[test]
    fn test_compile_injection() {
        let ast_expr = AstExpression::StringLiteral("Test injection".to_string());
        let ast_stmt = AstStatement::Injection(ast_expr);
        let compiled = Compiler::compile_statement(&ast_stmt).unwrap();

        let mut context = Context::new();
        let result = compiled.evaluate(&mut context).unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Test injection"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events.len(), 1);
        assert_eq!(context.events[0].message, "Test injection");
    }

    #[test]
    fn test_compile_variable() {
        let ast_expr = AstExpression::Variable("test_var".to_string());
        let compiled = Compiler::compile_expression(&ast_expr).unwrap();

        let mut context = Context::new();
        context.set_variable(
            "test_var".to_string(),
            ExprResult::String("variable_value".to_string()),
        );

        let result = compiled.evaluate(&mut context).unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "variable_value"),
            _ => panic!("Expected string result"),
        }
    }
}
