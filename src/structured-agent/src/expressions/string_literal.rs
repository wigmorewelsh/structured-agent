use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct StringLiteralExpr {
    pub value: String,
}

#[async_trait(?Send)]
impl Expression for StringLiteralExpr {
    async fn evaluate(&self, _context: &mut Context) -> Result<ExprResult, String> {
        Ok(ExprResult::String(self.value.clone()))
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
    async fn test_string_literal_evaluation() {
        let expr = StringLiteralExpr {
            value: "Hello, world!".to_string(),
        };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = expr.evaluate(&mut context).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Hello, world!"),
            _ => panic!("Expected string result"),
        }
    }

    #[test]
    fn test_string_literal_return_type() {
        let expr = StringLiteralExpr {
            value: "test".to_string(),
        };

        let return_type = expr.return_type();
        assert_eq!(return_type.name, "String");
    }

    #[tokio::test]
    async fn test_string_literal_clone() {
        let expr = StringLiteralExpr {
            value: "test".to_string(),
        };

        let cloned = expr.clone_box();
        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);

        let result1 = expr.evaluate(&mut context).await.unwrap();
        let result2 = cloned.evaluate(&mut context).await.unwrap();

        assert_eq!(result1, result2);
    }
}
