use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct VariableExpr {
    pub name: String,
}

#[async_trait(?Send)]
impl Expression for VariableExpr {
    async fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        context
            .get_variable(&self.name)
            .cloned()
            .ok_or_else(|| format!("Variable '{}' not found", self.name))
    }

    fn return_type(&self) -> Type {
        Type::string()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_variable_found() {
        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        context.set_variable(
            "test_var".to_string(),
            ExprResult::String("test_value".to_string()),
        );

        let expr = VariableExpr {
            name: "test_var".to_string(),
        };

        let result = expr.evaluate(&mut context).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "test_value"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_variable_not_found() {
        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let expr = VariableExpr {
            name: "unknown_var".to_string(),
        };

        let result = expr.evaluate(&mut context).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Variable 'unknown_var' not found")
        );
    }

    #[test]
    fn test_variable_return_type() {
        let expr = VariableExpr {
            name: "test".to_string(),
        };

        let return_type = expr.return_type();
        assert_eq!(return_type.name, "String");
    }

    #[tokio::test]
    async fn test_variable_clone() {
        let expr = VariableExpr {
            name: "test_var".to_string(),
        };

        let cloned = expr.clone_box();

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result1 = expr.evaluate(&mut context).await;
        let result2 = cloned.evaluate(&mut context).await;

        assert!(result1.is_err());
        assert!(result2.is_err());
    }
}
