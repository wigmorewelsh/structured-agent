use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

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
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
        let result = self.inner.evaluate(context.clone()).await?;

        match &result {
            ExprResult::String(s) => context.add_event(s.clone()),
            ExprResult::Unit => {}
            ExprResult::Boolean(b) => context.add_event(b.to_string()),
        }

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
    use crate::expressions::StringLiteralExpr;
    use crate::runtime::Runtime;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_injection_evaluation() {
        let inner = StringLiteralExpr {
            value: "Injected content".to_string(),
        };

        let expr = InjectionExpr {
            inner: Box::new(inner),
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context.clone()).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Injected content"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events.borrow().len(), 1);
        assert_eq!(context.events.borrow()[0].message, "Injected content");
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
        assert_eq!(return_type.name, "String");
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
        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));

        let result1 = expr.evaluate(context.clone()).await.unwrap();
        let result2 = cloned.evaluate(context.clone()).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(context.events.borrow().len(), 2);
        assert_eq!(context.events.borrow()[0].message, "test content");
        assert_eq!(context.events.borrow()[1].message, "test content");
    }
}
