use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{Expression, Type};
use arrow::array::Array;
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use tracing::{debug, info};

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
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let name = self.inner.name();
        let result = self.inner.evaluate(context.clone()).await?;

        fn format_expr_result(result: &ExpressionValue) -> String {
            match result {
                ExpressionValue::String(s) => s.clone(),
                ExpressionValue::Unit => "()".to_string(),
                ExpressionValue::Boolean(b) => b.to_string(),
                ExpressionValue::List(list) => {
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
                ExpressionValue::Option(opt) => match opt {
                    Some(inner) => format!("Some({})", format_expr_result(inner)),
                    None => "None".to_string(),
                },
            }
        }

        match &result.value {
            ExpressionValue::Unit => {}
            _ => {
                let formatted = format_expr_result(&result.value);
                let event = to_event(formatted.clone(), name.clone());
                context.add_event(event.clone());
                info!("{}", event);
                debug!(
                    name = ?name,
                    value_type = %result.value.type_name(),
                    "injection"
                );
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
        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        let result1 = expr.evaluate(context.clone()).await.unwrap();
        let result2 = cloned.evaluate(context.clone()).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(context.events_count(), 2);
        assert_eq!(context.get_event(0).unwrap().message, "test content");
        assert_eq!(context.get_event(1).unwrap().message, "test content");
    }
}
