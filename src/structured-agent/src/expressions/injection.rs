use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;

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
    async fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let result = self.inner.evaluate(context).await?;

        match &result {
            ExprResult::String(s) => context.add_event(s.clone()),
            ExprResult::Unit => {}
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
        let mut context = Context::with_runtime(runtime);
        let result = expr.evaluate(&mut context).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Injected content"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events.len(), 1);
        assert_eq!(context.events[0].message, "Injected content");
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
        let mut context = Context::with_runtime(runtime);

        let result1 = expr.evaluate(&mut context).await.unwrap();
        let result2 = cloned.evaluate(&mut context).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(context.events.len(), 2);
        assert_eq!(context.events[0].message, "test content");
        assert_eq!(context.events[1].message, "test content");
    }
}
