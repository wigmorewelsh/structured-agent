use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BooleanLiteralExpr {
    pub value: bool,
}

#[async_trait(?Send)]
impl Expression for BooleanLiteralExpr {
    async fn evaluate(&self, _context: Arc<Context>) -> Result<ExpressionResult, String> {
        Ok(ExpressionResult::new(ExpressionValue::Boolean(self.value)))
    }

    fn return_type(&self) -> Type {
        Type::boolean()
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
    use crate::compiler::CompilationUnit;
    use crate::runtime::Runtime;
    use std::rc::Rc;

    fn test_runtime() -> Runtime {
        let program = CompilationUnit::from_string("fn main(): () {}".to_string());
        Runtime::builder(program).build()
    }

    #[tokio::test]
    async fn test_boolean_literal_true_evaluation() {
        let expr = BooleanLiteralExpr { value: true };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::Boolean(b) => assert_eq!(b, true),
            _ => panic!("Expected boolean result"),
        }
    }

    #[tokio::test]
    async fn test_boolean_literal_false_evaluation() {
        let expr = BooleanLiteralExpr { value: false };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::Boolean(b) => assert_eq!(b, false),
            _ => panic!("Expected boolean result"),
        }
    }

    #[test]
    fn test_boolean_literal_return_type() {
        let expr = BooleanLiteralExpr { value: true };

        let return_type = expr.return_type();
        assert_eq!(return_type.name(), "Boolean");
    }

    #[tokio::test]
    async fn test_boolean_literal_clone() {
        let expr = BooleanLiteralExpr { value: true };

        let cloned = expr.clone_box();
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        let result1 = expr.evaluate(context.clone()).await.unwrap();
        let result2 = cloned.evaluate(context.clone()).await.unwrap();

        assert_eq!(result1, result2);
    }
}
