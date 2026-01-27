use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub struct IfElseExpr {
    pub condition: Box<dyn Expression>,
    pub then_expr: Box<dyn Expression>,
    pub else_expr: Box<dyn Expression>,
}

#[async_trait(?Send)]
impl Expression for IfElseExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
        let condition_result = self.condition.evaluate(context.clone()).await?;
        let condition_value = condition_result
            .as_boolean()
            .map_err(|_| "if-else condition must be a boolean expression".to_string())?;

        if condition_value {
            self.then_expr.evaluate(context).await
        } else {
            self.else_expr.evaluate(context).await
        }
    }

    fn return_type(&self) -> Type {
        self.then_expr.return_type()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(IfElseExpr {
            condition: self.condition.clone_box(),
            then_expr: self.then_expr.clone_box(),
            else_expr: self.else_expr.clone_box(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::{BooleanLiteralExpr, StringLiteralExpr};
    use crate::runtime::Runtime;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_if_else_true_condition() {
        let condition = Box::new(BooleanLiteralExpr { value: true });
        let then_expr = Box::new(StringLiteralExpr {
            value: "then branch".to_string(),
        });
        let else_expr = Box::new(StringLiteralExpr {
            value: "else branch".to_string(),
        });

        let if_else_expr = IfElseExpr {
            condition,
            then_expr,
            else_expr,
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = if_else_expr.evaluate(context).await.unwrap();

        assert_eq!(result, ExprResult::String("then branch".to_string()));
    }

    #[tokio::test]
    async fn test_if_else_false_condition() {
        let condition = Box::new(BooleanLiteralExpr { value: false });
        let then_expr = Box::new(StringLiteralExpr {
            value: "then branch".to_string(),
        });
        let else_expr = Box::new(StringLiteralExpr {
            value: "else branch".to_string(),
        });

        let if_else_expr = IfElseExpr {
            condition,
            then_expr,
            else_expr,
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = if_else_expr.evaluate(context).await.unwrap();

        assert_eq!(result, ExprResult::String("else branch".to_string()));
    }

    #[tokio::test]
    async fn test_if_else_non_boolean_condition() {
        let condition = Box::new(StringLiteralExpr {
            value: "not a boolean".to_string(),
        });
        let then_expr = Box::new(StringLiteralExpr {
            value: "then branch".to_string(),
        });
        let else_expr = Box::new(StringLiteralExpr {
            value: "else branch".to_string(),
        });

        let if_else_expr = IfElseExpr {
            condition,
            then_expr,
            else_expr,
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = if_else_expr.evaluate(context).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "if-else condition must be a boolean expression"
        );
    }

    #[test]
    fn test_if_else_return_type() {
        let condition = Box::new(BooleanLiteralExpr { value: true });
        let then_expr = Box::new(StringLiteralExpr {
            value: "then".to_string(),
        });
        let else_expr = Box::new(StringLiteralExpr {
            value: "else".to_string(),
        });

        let if_else_expr = IfElseExpr {
            condition,
            then_expr,
            else_expr,
        };

        assert_eq!(if_else_expr.return_type().name(), "String");
    }

    #[tokio::test]
    async fn test_nested_if_else() {
        let inner_if_else = IfElseExpr {
            condition: Box::new(BooleanLiteralExpr { value: false }),
            then_expr: Box::new(StringLiteralExpr {
                value: "inner then".to_string(),
            }),
            else_expr: Box::new(StringLiteralExpr {
                value: "inner else".to_string(),
            }),
        };

        let outer_if_else = IfElseExpr {
            condition: Box::new(BooleanLiteralExpr { value: true }),
            then_expr: Box::new(inner_if_else),
            else_expr: Box::new(StringLiteralExpr {
                value: "outer else".to_string(),
            }),
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = outer_if_else.evaluate(context).await.unwrap();

        assert_eq!(result, ExprResult::String("inner else".to_string()));
    }

    #[tokio::test]
    async fn test_if_else_with_variables() {
        use crate::expressions::VariableExpr;

        let condition = Box::new(BooleanLiteralExpr { value: true });
        let then_expr = Box::new(VariableExpr {
            name: "ready".to_string(),
        });
        let else_expr = Box::new(StringLiteralExpr {
            value: "fallback".to_string(),
        });

        let if_else_expr = IfElseExpr {
            condition,
            then_expr,
            else_expr,
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));

        context.declare_variable(
            "ready".to_string(),
            ExprResult::String("success".to_string()),
        );

        let result = if_else_expr.evaluate(context).await.unwrap();

        assert_eq!(result, ExprResult::String("success".to_string()));
    }
}
