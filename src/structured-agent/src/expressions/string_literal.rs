use crate::types::{Context, ExprResult, Expression, Type};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct StringLiteralExpr {
    pub value: String,
}

impl Expression for StringLiteralExpr {
    fn evaluate(&self, _context: &mut Context) -> Result<ExprResult, String> {
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

    #[test]
    fn test_string_literal_evaluation() {
        let expr = StringLiteralExpr {
            value: "Hello, world!".to_string(),
        };

        let mut context = Context::new();
        let result = expr.evaluate(&mut context).unwrap();

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

    #[test]
    fn test_string_literal_clone() {
        let expr = StringLiteralExpr {
            value: "test".to_string(),
        };

        let cloned = expr.clone_box();
        let mut context = Context::new();

        let result1 = expr.evaluate(&mut context).unwrap();
        let result2 = cloned.evaluate(&mut context).unwrap();

        assert_eq!(result1, result2);
    }
}
