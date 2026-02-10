use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub struct ReturnExpr {
    pub expression: Box<dyn Expression>,
}

impl ReturnExpr {
    pub fn new(expression: Box<dyn Expression>) -> Self {
        Self { expression }
    }
}

impl Clone for ReturnExpr {
    fn clone(&self) -> Self {
        Self {
            expression: self.expression.clone_box(),
        }
    }
}

impl PartialEq for ReturnExpr {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.expression.as_ref(), other.expression.as_ref())
    }
}

#[async_trait(?Send)]
impl Expression for ReturnExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let result = self.expression.evaluate(context.clone()).await?;
        context.set_return_value(result.clone());
        Ok(result)
    }

    fn return_type(&self) -> Type {
        self.expression.return_type()
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
    use crate::expressions::StringLiteralExpr;
    use crate::runtime::Runtime;
    use std::rc::Rc;

    fn test_runtime() -> Runtime {
        let program = CompilationUnit::from_string("fn main(): () {}".to_string());
        Runtime::builder(program).build()
    }

    #[tokio::test]
    async fn test_return_sets_function_level_variable() {
        let return_expr = ReturnExpr::new(Box::new(StringLiteralExpr {
            value: "test_value".to_string(),
        }));

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        let result = return_expr.evaluate(context.clone()).await.unwrap();

        assert_eq!(
            result.value,
            ExpressionValue::String("test_value".to_string())
        );
        assert_eq!(
            context.get_return_value().unwrap().value,
            ExpressionValue::String("test_value".to_string())
        );
    }

    #[tokio::test]
    async fn test_return_in_nested_context_sets_function_level() {
        let return_expr = ReturnExpr::new(Box::new(StringLiteralExpr {
            value: "nested_return".to_string(),
        }));

        let runtime = Rc::new(test_runtime());
        let function_context = Arc::new(Context::with_runtime(runtime.clone()));
        let nested_context = Arc::new(Context::create_child(
            function_context.clone(),
            false,
            runtime,
        ));

        let result = return_expr.evaluate(nested_context.clone()).await.unwrap();

        assert_eq!(
            result.value,
            ExpressionValue::String("nested_return".to_string())
        );
        assert_eq!(
            function_context.get_return_value().unwrap().value,
            ExpressionValue::String("nested_return".to_string())
        );
        assert!(!nested_context.has_return_value());
    }

    #[test]
    fn test_return_type_matches_expression() {
        let return_expr = ReturnExpr::new(Box::new(StringLiteralExpr {
            value: "test".to_string(),
        }));

        assert_eq!(return_expr.return_type(), Type::string());
    }
}
