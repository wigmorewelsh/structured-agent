use crate::types::{Context, ExprResult, Expression, Type};
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

impl Expression for InjectionExpr {
    fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let result = self.inner.evaluate(context)?;

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

    #[test]
    fn test_injection_evaluation() {
        let inner = StringLiteralExpr {
            value: "Injected content".to_string(),
        };

        let expr = InjectionExpr {
            inner: Box::new(inner),
        };

        let mut context = Context::new();
        let result = expr.evaluate(&mut context).unwrap();

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

    #[test]
    fn test_injection_clone() {
        let inner = StringLiteralExpr {
            value: "test content".to_string(),
        };

        let expr = InjectionExpr {
            inner: Box::new(inner),
        };

        let cloned = expr.clone_box();
        let mut context = Context::new();

        let result1 = expr.evaluate(&mut context).unwrap();
        let result2 = cloned.evaluate(&mut context).unwrap();

        assert_eq!(result1, result2);
        assert_eq!(context.events.len(), 2);
        assert_eq!(context.events[0].message, "test content");
        assert_eq!(context.events[1].message, "test content");
    }
}
