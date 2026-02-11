use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use tracing::debug;

pub struct InjectionExpr {
    pub inner: Box<dyn Expression>,
}

impl std::fmt::Debug for InjectionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InjectionExpr")
            .field("inner", &"<expression>")
            .finish()
    }
}

#[async_trait(?Send)]
impl Expression for InjectionExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let result = self.inner.evaluate(context.clone()).await?;

        let event_name = result
            .name
            .clone()
            .or_else(|| self.inner.name().map(|s| s.to_string()));
        context.add_event(
            result.value.clone(),
            event_name.clone(),
            result.params.clone(),
        );
        debug!(
            name = ?event_name,
            value_type = %result.value.type_name(),
            "injection"
        );

        Ok(result)
    }

    fn return_type(&self) -> Type {
        self.inner.return_type()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(InjectionExpr {
            inner: self.inner.clone_box(),
        })
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
    async fn test_injection_evaluation() {
        let inner = StringLiteralExpr {
            value: "Injected content".to_string(),
        };

        let expr = InjectionExpr {
            inner: Box::new(inner),
        };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context.clone()).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "Injected content"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events_count(), 1);
        let event = context.get_event(0).unwrap();
        match event.content {
            ExpressionValue::String(s) => assert_eq!(s, "Injected content"),
            _ => panic!("Expected string content"),
        }
    }

    #[test]
    fn test_injection_return_type() {
        let inner = StringLiteralExpr {
            value: "test".to_string(),
        };

        let expr = InjectionExpr {
            inner: Box::new(inner),
        };

        let return_type = expr.return_type();
        assert_eq!(return_type.name(), "String");
    }

    #[tokio::test]
    async fn test_injection_clone() {
        let inner = StringLiteralExpr {
            value: "test content".to_string(),
        };

        let expr = InjectionExpr {
            inner: Box::new(inner),
        };

        let cloned = expr.clone_box();
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        let result1 = expr.evaluate(context.clone()).await.unwrap();
        let result2 = cloned.evaluate(context.clone()).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(context.events_count(), 2);
        match &context.get_event(0).unwrap().content {
            ExpressionValue::String(s) => assert_eq!(s, "test content"),
            _ => panic!("Expected string content"),
        }
        match &context.get_event(1).unwrap().content {
            ExpressionValue::String(s) => assert_eq!(s, "test content"),
            _ => panic!("Expected string content"),
        }
    }
}
