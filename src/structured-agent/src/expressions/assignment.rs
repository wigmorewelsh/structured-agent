use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;

#[derive(Debug)]
pub struct AssignmentExpr {
    pub variable: String,
    pub expression: Box<dyn Expression>,
}

#[async_trait(?Send)]
impl Expression for AssignmentExpr {
    async fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let value = self.expression.evaluate(context).await?;
        context.set_variable(self.variable.clone(), value);
        Ok(ExprResult::Unit)
    }

    fn return_type(&self) -> Type {
        Type::unit()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(AssignmentExpr {
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
    async fn test_assignment_evaluation() {
        let expr = AssignmentExpr {
            variable: "test_var".to_string(),
            expression: Box::new(StringLiteralExpr {
                value: "test_value".to_string(),
            }),
        };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = expr.evaluate(&mut context).await.unwrap();

        match result {
            ExprResult::Unit => {}
            _ => panic!("Expected unit result"),
        }

        assert_eq!(
            context.get_variable("test_var").unwrap(),
            &ExprResult::String("test_value".to_string())
        );
    }

    #[test]
    fn test_assignment_return_type() {
        let expr = AssignmentExpr {
            variable: "test".to_string(),
            expression: Box::new(StringLiteralExpr {
                value: "value".to_string(),
            }),
        };

        let return_type = expr.return_type();
        assert_eq!(return_type.name, "()");
    }

    #[tokio::test]
    async fn test_assignment_clone() {
        let expr = AssignmentExpr {
            variable: "var".to_string(),
            expression: Box::new(StringLiteralExpr {
                value: "value".to_string(),
            }),
        };

        let cloned = expr.clone_box();
        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);

        let result1 = expr.evaluate(&mut context).await.unwrap();
        let result2 = cloned.evaluate(&mut context).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(result1, ExprResult::Unit);
        assert_eq!(result2, ExprResult::Unit);
        assert_eq!(
            context.get_variable("var").unwrap(),
            &ExprResult::String("value".to_string())
        );
    }
}
