use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use arrow::array::Array;
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

fn to_event(message: String, name: Option<&str>) -> String {
    if let Some(name) = name {
        format!("<{}>\n{}\n</{}>", name, message, name)
    } else {
        message
    }
}

#[async_trait(?Send)]
impl Expression for InjectionExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
        let name = self.inner.name();
        let result = self.inner.evaluate(context.clone()).await?;

        fn format_expr_result(result: &ExprResult) -> String {
            match result {
                ExprResult::String(s) => s.clone(),
                ExprResult::Unit => "()".to_string(),
                ExprResult::Boolean(b) => b.to_string(),
                ExprResult::List(list) => {
                    if list.len() == 0 {
                        "[]".to_string()
                    } else {
                        let values = list.value(0);
                        if let Some(string_array) =
                            values.as_any().downcast_ref::<arrow::array::StringArray>()
                        {
                            let items: Vec<String> = (0..string_array.len())
                                .map(|i| format!("\"{}\"", string_array.value(i)))
                                .collect();
                            format!("[{}]", items.join(", "))
                        } else {
                            "[]".to_string()
                        }
                    }
                }
                ExprResult::Option(opt) => match opt {
                    Some(inner) => format!("Some({})", format_expr_result(inner)),
                    None => "None".to_string(),
                },
            }
        }

        match &result {
            ExprResult::String(s) => context.add_event(to_event(s.clone(), name.clone())),
            ExprResult::Unit => {}
            _ => {
                let formatted = format_expr_result(&result);
                context.add_event(to_event(formatted, name.clone()))
            }
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

        assert_eq!(context.events_count(), 1);
        assert_eq!(context.get_event(0).unwrap().message, "Injected content");
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
        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));

        let result1 = expr.evaluate(context.clone()).await.unwrap();
        let result2 = cloned.evaluate(context.clone()).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(context.events_count(), 2);
        assert_eq!(context.get_event(0).unwrap().message, "test content");
        assert_eq!(context.get_event(1).unwrap().message, "test content");
    }
}
