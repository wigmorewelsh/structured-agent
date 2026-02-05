use crate::runtime::{Context, ExprResult};
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
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
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
                    match elem {
                        ExprResult::String(s) => value_builder.append_value(&s),
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

        Ok(ExprResult::List(list_array))
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
    use crate::expressions::StringLiteralExpr;
    use crate::runtime::Runtime;
    use arrow::array::Array;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_empty_list_evaluation() {
        let expr = ListLiteralExpr {
            elements: vec![],
            element_type: Type::string(),
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await.unwrap();

        match result {
            ExprResult::List(list) => {
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

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await.unwrap();

        match result {
            ExprResult::List(list) => {
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
