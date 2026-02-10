use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub struct WhileExpr {
    pub condition: Box<dyn Expression>,
    pub body: Vec<Box<dyn Expression>>,
}

#[async_trait(?Send)]
impl Expression for WhileExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let mut iteration_count = 0;

        loop {
            iteration_count += 1;

            if iteration_count > 100 {
                return Err("While loop exceeded 100 iterations, likely infinite loop".to_string());
            }

            let condition_result = self.condition.evaluate(context.clone()).await?;

            let condition_value = condition_result
                .value
                .as_boolean()
                .map_err(|_| "while condition must be a boolean expression".to_string())?;

            if !condition_value {
                break;
            }

            let child_context = Arc::new(Context::create_child(
                context.clone(),
                false,
                context.runtime_rc(),
            ));

            for statement in &self.body {
                statement.evaluate(child_context.clone()).await?;

                if child_context.has_return_value() {
                    return Ok(ExpressionResult::new(ExpressionValue::Unit));
                }
            }
        }

        Ok(ExpressionResult::new(ExpressionValue::Unit))
    }

    fn return_type(&self) -> Type {
        Type::unit()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(WhileExpr {
            condition: self.condition.clone_box(),
            body: self.body.iter().map(|expr| expr.clone_box()).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompilationUnit;
    use crate::expressions::{
        AssignmentExpr, BooleanLiteralExpr, InjectionExpr, StringLiteralExpr,
        VariableAssignmentExpr, VariableExpr,
    };
    use crate::runtime::Runtime;
    use std::rc::Rc;

    fn test_runtime() -> Runtime {
        let program = CompilationUnit::from_string("fn main(): () {}".to_string());
        Runtime::builder(program).build()
    }

    #[tokio::test]
    async fn test_while_false_condition() {
        let condition = Box::new(BooleanLiteralExpr { value: false });
        let body = vec![Box::new(InjectionExpr {
            inner: Box::new(StringLiteralExpr {
                value: "never executed".to_string(),
            }),
        }) as Box<dyn Expression>];

        let while_expr = WhileExpr { condition, body };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = while_expr.evaluate(context.clone()).await.unwrap();

        assert_eq!(result.value, ExpressionValue::Unit);
        assert_eq!(context.events_count(), 0);
    }

    #[tokio::test]
    async fn test_while_non_boolean_condition() {
        let condition = Box::new(StringLiteralExpr {
            value: "not a boolean".to_string(),
        });
        let body = vec![];

        let while_expr = WhileExpr { condition, body };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = while_expr.evaluate(context).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "while condition must be a boolean expression"
        );
    }

    #[tokio::test]
    async fn test_while_with_variable_condition() {
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        context.declare_variable(
            "should_continue".to_string(),
            ExpressionResult::new(ExpressionValue::Boolean(true)),
        );

        let condition = Box::new(VariableExpr {
            name: "should_continue".to_string(),
        });

        let body = vec![
            Box::new(InjectionExpr {
                inner: Box::new(StringLiteralExpr {
                    value: "loop iteration".to_string(),
                }),
            }) as Box<dyn Expression>,
            Box::new(VariableAssignmentExpr {
                variable: "should_continue".to_string(),
                expression: Box::new(BooleanLiteralExpr { value: false }),
            }) as Box<dyn Expression>,
        ];

        let while_expr = WhileExpr { condition, body };
        let result = while_expr.evaluate(context.clone()).await.unwrap();

        assert_eq!(result.value, ExpressionValue::Unit);
        assert_eq!(context.events_count(), 0);
        assert_eq!(
            context.get_variable("should_continue").unwrap().value,
            ExpressionValue::Boolean(false)
        );
    }

    #[tokio::test]
    async fn test_while_variable_scoping() {
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        context.declare_variable(
            "outer_var".to_string(),
            ExpressionResult::new(ExpressionValue::String("outer_value".to_string())),
        );
        context.declare_variable(
            "should_continue".to_string(),
            ExpressionResult::new(ExpressionValue::Boolean(true)),
        );

        let condition = Box::new(VariableExpr {
            name: "should_continue".to_string(),
        });

        let body = vec![
            Box::new(AssignmentExpr {
                variable: "inner_var".to_string(),
                expression: Box::new(StringLiteralExpr {
                    value: "inner_value".to_string(),
                }),
            }) as Box<dyn Expression>,
            Box::new(VariableAssignmentExpr {
                variable: "should_continue".to_string(),
                expression: Box::new(BooleanLiteralExpr { value: false }),
            }) as Box<dyn Expression>,
        ];

        let while_expr = WhileExpr { condition, body };
        let result = while_expr.evaluate(context.clone()).await.unwrap();

        assert_eq!(result.value, ExpressionValue::Unit);

        assert_eq!(
            context.get_variable("outer_var").unwrap().value,
            ExpressionValue::String("outer_value".to_string())
        );

        assert_eq!(
            context.get_variable("should_continue").unwrap().value,
            ExpressionValue::Boolean(false)
        );

        assert!(context.get_variable("inner_var").is_none());
    }

    #[tokio::test]
    async fn test_nested_while_statements() {
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        context.declare_variable(
            "outer_continue".to_string(),
            ExpressionResult::new(ExpressionValue::Boolean(true)),
        );
        context.declare_variable(
            "inner_continue".to_string(),
            ExpressionResult::new(ExpressionValue::Boolean(true)),
        );

        let inner_while = WhileExpr {
            condition: Box::new(VariableExpr {
                name: "inner_continue".to_string(),
            }),
            body: vec![
                Box::new(InjectionExpr {
                    inner: Box::new(StringLiteralExpr {
                        value: "inner while executed".to_string(),
                    }),
                }) as Box<dyn Expression>,
                Box::new(VariableAssignmentExpr {
                    variable: "inner_continue".to_string(),
                    expression: Box::new(BooleanLiteralExpr { value: false }),
                }) as Box<dyn Expression>,
            ],
        };

        let outer_while = WhileExpr {
            condition: Box::new(VariableExpr {
                name: "outer_continue".to_string(),
            }),
            body: vec![
                Box::new(InjectionExpr {
                    inner: Box::new(StringLiteralExpr {
                        value: "outer while executed".to_string(),
                    }),
                }) as Box<dyn Expression>,
                Box::new(inner_while) as Box<dyn Expression>,
                Box::new(VariableAssignmentExpr {
                    variable: "outer_continue".to_string(),
                    expression: Box::new(BooleanLiteralExpr { value: false }),
                }) as Box<dyn Expression>,
            ],
        };

        let result = outer_while.evaluate(context.clone()).await.unwrap();

        assert_eq!(result.value, ExpressionValue::Unit);
        assert_eq!(context.events_count(), 0);
    }

    #[tokio::test]
    async fn test_while_can_access_parent_variables() {
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        context.declare_variable(
            "parent_var".to_string(),
            ExpressionResult::new(ExpressionValue::String("parent_value".to_string())),
        );
        context.declare_variable(
            "should_continue".to_string(),
            ExpressionResult::new(ExpressionValue::Boolean(true)),
        );

        let condition = Box::new(VariableExpr {
            name: "should_continue".to_string(),
        });

        let body = vec![
            Box::new(InjectionExpr {
                inner: Box::new(VariableExpr {
                    name: "parent_var".to_string(),
                }),
            }) as Box<dyn Expression>,
            Box::new(AssignmentExpr {
                variable: "local_var".to_string(),
                expression: Box::new(StringLiteralExpr {
                    value: "local_value".to_string(),
                }),
            }) as Box<dyn Expression>,
            Box::new(VariableAssignmentExpr {
                variable: "should_continue".to_string(),
                expression: Box::new(BooleanLiteralExpr { value: false }),
            }) as Box<dyn Expression>,
        ];

        let while_expr = WhileExpr { condition, body };
        let result = while_expr.evaluate(context.clone()).await.unwrap();

        assert_eq!(result.value, ExpressionValue::Unit);

        assert_eq!(context.events_count(), 0);

        assert_eq!(
            context.get_variable("parent_var").unwrap().value,
            ExpressionValue::String("parent_value".to_string())
        );

        assert!(context.get_variable("local_var").is_none());

        assert_eq!(
            context.get_variable("should_continue").unwrap().value,
            ExpressionValue::Boolean(false)
        );
    }
}
