use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PlaceholderExpr {}

#[async_trait(?Send)]
impl Expression for PlaceholderExpr {
    async fn evaluate(&self, _context: Arc<Context>) -> Result<ExprResult, String> {
        Err("Placeholder expressions should be replaced before evaluation".to_string())
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
    async fn test_placeholder_evaluation_fails() {
        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let expr = PlaceholderExpr {};

        let result = expr.evaluate(context).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Placeholder expressions should be replaced before evaluation")
        );
    }

    #[test]
    fn test_placeholder_return_type() {
        let expr = PlaceholderExpr {};
        let return_type = expr.return_type();
        assert_eq!(return_type.name(), "String");
    }

    #[test]
    fn test_placeholder_clone() {
        let expr = PlaceholderExpr {};
        let cloned = expr.clone_box();

        assert_eq!(expr.return_type().name(), cloned.return_type().name());
    }
}
