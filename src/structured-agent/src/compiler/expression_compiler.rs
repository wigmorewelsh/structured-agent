use crate::ast;
use crate::compiler::FunctionCompiler;
use crate::expressions::{
    AssignmentExpr, BooleanLiteralExpr, CallExpr, FunctionExpr, IfElseExpr, IfExpr, InjectionExpr,
    ListLiteralExpr, PlaceholderExpr, ReturnExpr, SelectClauseExpr, SelectExpr, StringLiteralExpr,
    UnitLiteralExpr, VariableAssignmentExpr, VariableExpr, WhileExpr,
};
use crate::types::{Expression, ExternalFunctionDefinition, Parameter, Type};

pub struct ExpressionCompiler;

impl FunctionCompiler for ExpressionCompiler {
    fn compile_function(ast_func: &ast::Function) -> Result<FunctionExpr, String> {
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
                .map(|p| Parameter::new(p.name.clone(), convert_ast_type_to_type(&p.param_type)))
                .collect(),
            return_type: convert_ast_type_to_type(&ast_func.return_type),
            body: compiled_statements,
            documentation: ast_func.documentation.clone(),
        })
    }
}

impl ExpressionCompiler {
    fn compile_expression(ast_expr: &ast::Expression) -> Result<Box<dyn Expression>, String> {
        match ast_expr {
            ast::Expression::Call {
                function,
                arguments,
                ..
            } => {
                let compiled_args = arguments
                    .iter()
                    .map(|arg| Self::compile_expression(arg))
                    .collect::<Result<Vec<_>, String>>()?;

                Ok(Box::new(CallExpr {
                    function: function.clone(),
                    arguments: compiled_args,
                }))
            }
            ast::Expression::Placeholder { .. } => Ok(Box::new(PlaceholderExpr {})),
            ast::Expression::Select(select_expression) => {
                let compiled_clauses = select_expression
                    .clauses
                    .iter()
                    .map(|clause| {
                        let expression_to_run =
                            Self::compile_expression(&clause.expression_to_run)?;
                        let expression_next = Self::compile_expression(&clause.expression_next)?;
                        Ok(SelectClauseExpr {
                            expression_to_run,
                            result_variable: clause.result_variable.clone(),
                            expression_next,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;

                Ok(Box::new(SelectExpr {
                    clauses: compiled_clauses,
                }))
            }
            ast::Expression::Variable { name, .. } => {
                Ok(Box::new(VariableExpr { name: name.clone() }))
            }
            ast::Expression::StringLiteral { value, .. } => Ok(Box::new(StringLiteralExpr {
                value: value.clone(),
            })),
            ast::Expression::BooleanLiteral { value, .. } => {
                Ok(Box::new(BooleanLiteralExpr { value: *value }))
            }
            ast::Expression::UnitLiteral { .. } => Ok(Box::new(UnitLiteralExpr {})),
            ast::Expression::ListLiteral { elements, .. } => {
                if elements.is_empty() {
                    return Err("Cannot infer type of empty list literal".to_string());
                }

                let compiled_elements: Vec<Box<dyn Expression>> = elements
                    .iter()
                    .map(|elem| Self::compile_expression(elem))
                    .collect::<Result<Vec<_>, String>>()?;

                let element_type = compiled_elements[0].return_type();

                for elem in &compiled_elements {
                    if elem.return_type() != element_type {
                        return Err("All list elements must have the same type".to_string());
                    }
                }

                Ok(Box::new(ListLiteralExpr {
                    elements: compiled_elements,
                    element_type,
                }))
            }
            ast::Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let compiled_condition = Self::compile_expression(condition)?;
                let compiled_then = Self::compile_expression(then_expr)?;
                let compiled_else = Self::compile_expression(else_expr)?;

                Ok(Box::new(IfElseExpr {
                    condition: compiled_condition,
                    then_expr: compiled_then,
                    else_expr: compiled_else,
                }))
            }
        }
    }

    fn compile_statement(ast_stmt: &ast::Statement) -> Result<Box<dyn Expression>, String> {
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
                ..
            } => {
                let compiled_expression = Self::compile_expression(expression)?;
                Ok(Box::new(AssignmentExpr {
                    variable: variable.clone(),
                    expression: compiled_expression,
                }))
            }
            ast::Statement::VariableAssignment {
                variable,
                expression,
                ..
            } => {
                let compiled_expression = Self::compile_expression(expression)?;
                Ok(Box::new(VariableAssignmentExpr {
                    variable: variable.clone(),
                    expression: compiled_expression,
                }))
            }
            ast::Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                let compiled_condition = Self::compile_expression(condition)?;
                let compiled_body = body
                    .iter()
                    .map(|stmt| Self::compile_statement(stmt))
                    .collect::<Result<Vec<_>, String>>()?;
                let compiled_else = match else_body {
                    Some(else_stmts) => Some(
                        else_stmts
                            .iter()
                            .map(|stmt| Self::compile_statement(stmt))
                            .collect::<Result<Vec<_>, String>>()?,
                    ),
                    None => None,
                };
                Ok(Box::new(IfExpr {
                    condition: compiled_condition,
                    body: compiled_body,
                    else_body: compiled_else,
                }))
            }
            ast::Statement::While {
                condition, body, ..
            } => {
                let compiled_condition = Self::compile_expression(condition)?;

                let mut compiled_body = Vec::new();
                for stmt in body.iter() {
                    let compiled_stmt = Self::compile_statement(stmt)?;
                    compiled_body.push(compiled_stmt);
                }

                Ok(Box::new(WhileExpr {
                    condition: compiled_condition,
                    body: compiled_body,
                }))
            }
            ast::Statement::Return(expr) => {
                let compiled_expression = Self::compile_expression(expr)?;
                Ok(Box::new(ReturnExpr::new(compiled_expression)))
            }
            ast::Statement::ExpressionStatement(expr) => Self::compile_expression(expr),
        }
    }
}

pub fn compile_external_function(
    ast_ext_func: &ast::ExternalFunction,
) -> Result<ExternalFunctionDefinition, String> {
    let parameters = ast_ext_func
        .parameters
        .iter()
        .map(|p| Parameter::new(p.name.clone(), convert_ast_type_to_type(&p.param_type)))
        .collect();

    Ok(ExternalFunctionDefinition::new(
        ast_ext_func.name.clone(),
        parameters,
        convert_ast_type_to_type(&ast_ext_func.return_type),
    ))
}

fn convert_ast_type_to_type(ast_type: &ast::Type) -> Type {
    match ast_type {
        ast::Type::Unit => Type::unit(),
        ast::Type::Boolean => Type::boolean(),
        ast::Type::String => Type::string(),
        ast::Type::List(inner) => Type::list(convert_ast_type_to_type(inner)),
        ast::Type::Option(inner) => Type::option(convert_ast_type_to_type(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expression as AstExpression, Statement as AstStatement};
    use crate::compiler::CompilationUnit;
    use crate::runtime::{Context, ExpressionResult, ExpressionValue, Runtime};
    use std::rc::Rc;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_compile_string_literal() {
        let ast_expr = AstExpression::StringLiteral {
            value: "Hello".to_string(),
            span: crate::types::Span::dummy(),
        };
        let compiled = ExpressionCompiler::compile_expression(&ast_expr).unwrap();

        let dummy_program = CompilationUnit::from_string("fn main(): () {}".to_string());
        let runtime = Rc::new(Runtime::builder(dummy_program).build());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = compiled.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_compile_injection() {
        let ast_expr = AstExpression::StringLiteral {
            value: "Test injection".to_string(),
            span: crate::types::Span::dummy(),
        };
        let ast_stmt = AstStatement::Injection(ast_expr);
        let compiled = ExpressionCompiler::compile_statement(&ast_stmt).unwrap();

        let dummy_program = CompilationUnit::from_string("fn main(): () {}".to_string());
        let runtime = Rc::new(Runtime::builder(dummy_program).build());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = compiled.evaluate(context.clone()).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "Test injection"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events_count(), 1);
        let event = context.get_event(0).unwrap();
        match event.content {
            ExpressionValue::String(s) => assert_eq!(s, "Test injection"),
            _ => panic!("Expected string content in event"),
        }
    }

    #[tokio::test]
    async fn test_compile_variable() {
        let ast_expr = AstExpression::Variable {
            name: "test_var".to_string(),
            span: crate::types::Span::dummy(),
        };
        let compiled = ExpressionCompiler::compile_expression(&ast_expr).unwrap();

        let dummy_program = CompilationUnit::from_string("fn main(): () {}".to_string());
        let runtime = Rc::new(Runtime::builder(dummy_program).build());
        let context = Arc::new(Context::with_runtime(runtime));
        context.declare_variable(
            "test_var".to_string(),
            ExpressionResult::new(ExpressionValue::String("variable_value".to_string())),
        );

        let result = compiled.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "variable_value"),
            _ => panic!("Expected string result"),
        }
    }
}
