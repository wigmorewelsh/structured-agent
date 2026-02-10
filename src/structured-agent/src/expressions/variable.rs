use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct VariableExpr {
    pub name: String,
}

#[async_trait(?Send)]
impl Expression for VariableExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let value = context
            .get_variable(&self.name)
            .ok_or_else(|| format!("Variable '{}' not found", self.name))?;
        Ok(ExpressionResult::new(value))
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

    fn name(&self) -> Option<&str> {
        Some(self.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompilationUnit;
    use crate::runtime::Runtime;
    use std::rc::Rc;

    fn test_runtime() -> Runtime {
        let program = CompilationUnit::from_string("fn main(): () {}".to_string());
        Runtime::builder(program).build()
    }

    #[tokio::test]
    async fn test_variable_found() {
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        context.declare_variable(
            "test_var".to_string(),
            ExpressionValue::String("test_value".to_string()),
        );

        let expr = VariableExpr {
            name: "test_var".to_string(),
        };

        let result = expr.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "test_value"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_variable_not_found() {
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let expr = VariableExpr {
            name: "unknown_var".to_string(),
        };

        let result = expr.evaluate(context).await;
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
        assert_eq!(return_type.name(), "String");
    }

    #[tokio::test]
    async fn test_variable_clone() {
        let expr = VariableExpr {
            name: "test_var".to_string(),
        };

        let cloned = expr.clone_box();

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result1 = expr.evaluate(context.clone()).await;
        let result2 = cloned.evaluate(context.clone()).await;

        assert!(result1.is_err());
        assert!(result2.is_err());
    }
}
