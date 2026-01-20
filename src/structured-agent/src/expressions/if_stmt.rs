use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;

#[derive(Debug)]
pub struct IfExpr {
    pub condition: Box<dyn Expression>,
    pub body: Vec<Box<dyn Expression>>,
}

#[async_trait(?Send)]
impl Expression for IfExpr {
    async fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let condition_result = self.condition.evaluate(context).await?;
        let condition_value = condition_result
            .as_boolean()
            .map_err(|_| "if condition must be a boolean expression".to_string())?;

        if condition_value {
            for statement in &self.body {
                statement.evaluate(context).await?;
            }
        }
        Ok(ExprResult::Unit)
    }

    fn return_type(&self) -> Type {
        Type::unit()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(IfExpr {
            condition: self.condition.clone_box(),
            body: self.body.iter().map(|expr| expr.clone_box()).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::{BooleanLiteralExpr, InjectionExpr, StringLiteralExpr};
    use crate::runtime::Runtime;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_if_true_condition() {
        let condition = Box::new(BooleanLiteralExpr { value: true });
        let body = vec![Box::new(InjectionExpr {
            inner: Box::new(StringLiteralExpr {
                value: "executed".to_string(),
            }),
        }) as Box<dyn Expression>];

        let if_expr = IfExpr { condition, body };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = if_expr.evaluate(&mut context).await.unwrap();

        assert_eq!(result, ExprResult::Unit);
        assert_eq!(context.events.len(), 1);
        assert_eq!(context.events[0].message, "executed");
    }

    #[tokio::test]
    async fn test_if_false_condition() {
        let condition = Box::new(BooleanLiteralExpr { value: false });
        let body = vec![Box::new(InjectionExpr {
            inner: Box::new(StringLiteralExpr {
                value: "not executed".to_string(),
            }),
        }) as Box<dyn Expression>];

        let if_expr = IfExpr { condition, body };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = if_expr.evaluate(&mut context).await.unwrap();

        assert_eq!(result, ExprResult::Unit);
        assert_eq!(context.events.len(), 0);
    }

    #[tokio::test]
    async fn test_if_non_boolean_condition() {
        let condition = Box::new(StringLiteralExpr {
            value: "not a boolean".to_string(),
        });
        let body = vec![];

        let if_expr = IfExpr { condition, body };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = if_expr.evaluate(&mut context).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "if condition must be a boolean expression"
        );
    }

    #[test]
    fn test_if_return_type() {
        let condition = Box::new(BooleanLiteralExpr { value: true });
        let body = vec![];
        let if_expr = IfExpr { condition, body };

        assert_eq!(if_expr.return_type().name, "()");
    }

    #[tokio::test]
    async fn test_if_variable_scoping() {
        use crate::expressions::AssignmentExpr;

        let condition = Box::new(BooleanLiteralExpr { value: true });
        let body = vec![Box::new(AssignmentExpr {
            variable: "inner_var".to_string(),
            expression: Box::new(StringLiteralExpr {
                value: "inner_value".to_string(),
            }),
        }) as Box<dyn Expression>];

        let if_expr = IfExpr { condition, body };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);

        // Set outer variable
        context.set_variable(
            "outer_var".to_string(),
            ExprResult::String("outer_value".to_string()),
        );

        let result = if_expr.evaluate(&mut context).await.unwrap();
        assert_eq!(result, ExprResult::Unit);

        // Outer variable should still exist
        assert_eq!(
            context.get_variable("outer_var").unwrap(),
            &ExprResult::String("outer_value".to_string())
        );

        // Inner variable should now exist in the same context
        assert_eq!(
            context.get_variable("inner_var").unwrap(),
            &ExprResult::String("inner_value".to_string())
        );
    }

    #[tokio::test]
    async fn test_nested_if_statements() {
        let inner_if = IfExpr {
            condition: Box::new(BooleanLiteralExpr { value: true }),
            body: vec![Box::new(InjectionExpr {
                inner: Box::new(StringLiteralExpr {
                    value: "inner if executed".to_string(),
                }),
            }) as Box<dyn Expression>],
        };

        let outer_if = IfExpr {
            condition: Box::new(BooleanLiteralExpr { value: true }),
            body: vec![
                Box::new(InjectionExpr {
                    inner: Box::new(StringLiteralExpr {
                        value: "outer if executed".to_string(),
                    }),
                }) as Box<dyn Expression>,
                Box::new(inner_if) as Box<dyn Expression>,
            ],
        };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = outer_if.evaluate(&mut context).await.unwrap();

        assert_eq!(result, ExprResult::Unit);
        assert_eq!(context.events.len(), 2);
        assert_eq!(context.events[0].message, "outer if executed");
        assert_eq!(context.events[1].message, "inner if executed");
    }

    #[tokio::test]
    async fn test_if_can_access_parent_variables() {
        use crate::expressions::{AssignmentExpr, VariableExpr};

        let condition = Box::new(BooleanLiteralExpr { value: true });
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
        ];

        let if_expr = IfExpr { condition, body };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);

        // Set parent variable
        context.set_variable(
            "parent_var".to_string(),
            ExprResult::String("parent_value".to_string()),
        );

        let result = if_expr.evaluate(&mut context).await.unwrap();
        assert_eq!(result, ExprResult::Unit);

        // Should have injected parent variable value
        assert_eq!(context.events.len(), 1);
        assert_eq!(context.events[0].message, "parent_value");

        // Parent variable should still exist
        assert_eq!(
            context.get_variable("parent_var").unwrap(),
            &ExprResult::String("parent_value".to_string())
        );

        // Local variable should now exist in the same context
        assert_eq!(
            context.get_variable("local_var").unwrap(),
            &ExprResult::String("local_value".to_string())
        );
    }
}
