use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct UnitLiteralExpr {}

#[async_trait(?Send)]
impl Expression for UnitLiteralExpr {
    async fn evaluate(&self, _context: Arc<Context>) -> Result<ExpressionResult, String> {
        Ok(ExpressionResult::new(ExpressionValue::Unit))
    }

    fn return_type(&self) -> Type {
        Type::unit()
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
    async fn test_unit_literal_evaluation() {
        let expr = UnitLiteralExpr {};

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::Unit => {}
            _ => panic!("Expected unit result"),
        }
    }

    #[test]
    fn test_unit_literal_return_type() {
        let expr = UnitLiteralExpr {};

        let return_type = expr.return_type();
        assert_eq!(return_type.name(), "()");
    }

    #[tokio::test]
    async fn test_unit_literal_clone() {
        let expr = UnitLiteralExpr {};

        let cloned = expr.clone_box();
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        let result1 = expr.evaluate(context.clone()).await.unwrap();
        let result2 = cloned.evaluate(context.clone()).await.unwrap();

        assert_eq!(result1, result2);
    }
}
