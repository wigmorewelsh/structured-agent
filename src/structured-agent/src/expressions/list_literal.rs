use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{Expression, Type};
use arrow::array::{ListBuilder, StringBuilder};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub struct ListLiteralExpr {
    pub elements: Vec<Box<dyn Expression>>,
    pub element_type: Type,
}

#[async_trait(?Send)]
impl Expression for ListLiteralExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let mut evaluated_elements = Vec::new();
        for elem in &self.elements {
            let result = elem.evaluate(context.clone()).await?;
            evaluated_elements.push(result);
        }

        let list_array = match &self.element_type {
            Type::String => {
                let mut builder = ListBuilder::new(StringBuilder::new());

                for elem in evaluated_elements {
                    let value_builder = builder.values();
                    match elem.value {
                        ExpressionValue::String(s) => value_builder.append_value(&s),
                        _ => return Err("List element type mismatch".to_string()),
                    }
                    builder.append(true);
                }

                Arc::new(builder.finish())
            }
            _ => {
                return Err(format!(
                    "Unsupported list element type: {}",
                    self.element_type.name()
                ));
            }
        };

        Ok(ExpressionResult::new(ExpressionValue::List(list_array)))
    }

    fn return_type(&self) -> Type {
        Type::list(self.element_type.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        panic!("ListLiteralExpr cannot be cloned due to boxed trait objects")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompilationUnit;
    use crate::expressions::StringLiteralExpr;
    use crate::runtime::Runtime;
    use arrow::array::Array;
    use std::rc::Rc;

    fn test_runtime() -> Runtime {
        let program = CompilationUnit::from_string("fn main(): () {}".to_string());
        Runtime::builder(program).build()
    }

    #[tokio::test]
    async fn test_empty_list_evaluation() {
        let expr = ListLiteralExpr {
            elements: vec![],
            element_type: Type::string(),
        };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::List(list) => {
                assert_eq!(list.len(), 0);
            }
            _ => panic!("Expected list result"),
        }
    }

    #[tokio::test]
    async fn test_string_list_evaluation() {
        let expr = ListLiteralExpr {
            elements: vec![
                Box::new(StringLiteralExpr {
                    value: "hello".to_string(),
                }),
                Box::new(StringLiteralExpr {
                    value: "world".to_string(),
                }),
            ],
            element_type: Type::string(),
        };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::List(list) => {
                assert_eq!(list.len(), 2);
            }
            _ => panic!("Expected list result"),
        }
    }

    #[test]
    fn test_list_return_type() {
        let expr = ListLiteralExpr {
            elements: vec![],
            element_type: Type::string(),
        };

        let return_type = expr.return_type();
        assert_eq!(return_type.name(), "List<String>");
    }
}
