use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub struct VariableAssignmentExpr {
    pub variable: String,
    pub expression: Box<dyn Expression>,
}

#[async_trait(?Send)]
impl Expression for VariableAssignmentExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
        let value = self.expression.evaluate(context.clone()).await?;
        context.assign_variable(self.variable.clone(), value)?;
        Ok(ExprResult::Unit)
    }

    fn return_type(&self) -> Type {
        Type::unit()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(VariableAssignmentExpr {
            variable: self.variable.clone(),
            expression: self.expression.clone_box(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::StringLiteralExpr;
    use crate::runtime::Runtime;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_variable_assignment_updates_existing_variable() {
        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));

        context.declare_variable(
            "test_var".to_string(),
            ExprResult::String("initial".to_string()),
        );

        let assignment_expr = VariableAssignmentExpr {
            variable: "test_var".to_string(),
            expression: Box::new(StringLiteralExpr {
                value: "updated".to_string(),
            }),
        };

        let result = assignment_expr.evaluate(context.clone()).await.unwrap();

        assert_eq!(result, ExprResult::Unit);
        assert_eq!(
            context.get_variable("test_var").unwrap(),
            ExprResult::String("updated".to_string())
        );
    }

    #[tokio::test]
    async fn test_variable_assignment_fails_for_undeclared_variable() {
        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));

        let assignment_expr = VariableAssignmentExpr {
            variable: "nonexistent".to_string(),
            expression: Box::new(StringLiteralExpr {
                value: "value".to_string(),
            }),
        };

        let result = assignment_expr.evaluate(context).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Variable 'nonexistent' not found")
        );
    }

    #[tokio::test]
    async fn test_variable_assignment_in_child_context() {
        let runtime = Rc::new(Runtime::new());
        let parent_context = Arc::new(Context::with_runtime(runtime));

        parent_context.declare_variable(
            "shared_var".to_string(),
            ExprResult::String("parent".to_string()),
        );

        let child_context = Arc::new(Context::create_child(
            parent_context.clone(),
            false,
            parent_context.runtime_rc(),
        ));

        let assignment_expr = VariableAssignmentExpr {
            variable: "shared_var".to_string(),
            expression: Box::new(StringLiteralExpr {
                value: "child_updated".to_string(),
            }),
        };

        let result = assignment_expr
            .evaluate(child_context.clone())
            .await
            .unwrap();

        assert_eq!(result, ExprResult::Unit);
        assert_eq!(
            child_context.get_variable("shared_var").unwrap(),
            ExprResult::String("child_updated".to_string())
        );
        assert_eq!(
            parent_context.get_variable("shared_var").unwrap(),
            ExprResult::String("child_updated".to_string())
        );
    }

    #[tokio::test]
    async fn test_variable_assignment_respects_scope_boundaries() {
        let runtime = Rc::new(Runtime::new());
        let parent_context = Arc::new(Context::with_runtime(runtime));

        parent_context.declare_variable(
            "bounded_var".to_string(),
            ExprResult::String("parent".to_string()),
        );

        let child_context = Arc::new(Context::create_child(
            parent_context.clone(),
            true,
            parent_context.runtime_rc(),
        ));

        let assignment_expr = VariableAssignmentExpr {
            variable: "bounded_var".to_string(),
            expression: Box::new(StringLiteralExpr {
                value: "child_attempted".to_string(),
            }),
        };

        let result = assignment_expr.evaluate(child_context).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Variable 'bounded_var' not found")
        );

        assert_eq!(
            parent_context.get_variable("bounded_var").unwrap(),
            ExprResult::String("parent".to_string())
        );
    }
}
