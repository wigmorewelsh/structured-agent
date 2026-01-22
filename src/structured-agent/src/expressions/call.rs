use crate::expressions::PlaceholderExpr;
use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

pub struct CallExpr {
    pub function: String,
    pub arguments: Vec<Box<dyn Expression>>,
}

impl std::fmt::Debug for CallExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallExpr")
            .field("function", &self.function)
            .field("arguments", &format!("[{} args]", self.arguments.len()))
            .finish()
    }
}

#[async_trait(?Send)]
impl Expression for CallExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
        let function_info = context
            .runtime()
            .get_function(&self.function)
            .ok_or_else(|| format!("Unknown function: {}", self.function))?;

        let parameters = function_info.parameters().to_vec();

        if self.arguments.len() != parameters.len() {
            return Err(format!(
                "Function {} expects {} arguments, got {}",
                self.function,
                parameters.len(),
                self.arguments.len()
            ));
        }

        let mut args = Vec::new();
        for (i, arg) in self.arguments.iter().enumerate() {
            if arg.as_any().downcast_ref::<PlaceholderExpr>().is_some() {
                let (param_name, param_type) = &parameters[i];

                let value = context
                    .runtime()
                    .engine()
                    .fill_parameter(&context, param_name, param_type)
                    .await?;

                args.push(value);
            } else {
                args.push(arg.evaluate(context.clone()).await?);
            }
        }

        let function_context = Arc::new(Context::create_child(
            context.clone(),
            true,
            context.runtime_rc(),
        ));

        for (i, (param_name, _param_type)) in parameters.iter().enumerate() {
            function_context.declare_variable(param_name.clone(), args[i].clone());
        }

        let function_info = context
            .runtime()
            .get_function(&self.function)
            .expect("Function not found");

        let result = function_info.evaluate(function_context).await?;

        Ok(result)
    }

    fn return_type(&self) -> Type {
        Type::unit()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(CallExpr {
            function: self.function.clone(),
            arguments: self.arguments.iter().map(|arg| arg.clone_box()).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::{FunctionExpr, StringLiteralExpr};
    use crate::runtime::Runtime;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_unknown_method() {
        let expr = CallExpr {
            function: "unknown_method".to_string(),
            arguments: vec![],
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Unknown function: unknown_method")
        );
    }

    #[test]
    fn test_call_clone() {
        let expr = CallExpr {
            function: "method".to_string(),
            arguments: vec![],
        };

        let cloned = expr.clone_box();

        assert_eq!(
            expr.function,
            cloned.as_any().downcast_ref::<CallExpr>().unwrap().function
        );
    }

    #[tokio::test]
    async fn test_call_with_registry() {
        let mut runtime = Runtime::new();

        let function_info = FunctionExpr {
            name: "hello".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            body: vec![Box::new(StringLiteralExpr {
                value: "Hello, World!".to_string(),
            })],
            documentation: None,
        };
        runtime.register_function(function_info);

        let runtime = Rc::new(runtime);
        let context = Arc::new(Context::with_runtime(runtime));

        let expr = CallExpr {
            function: "hello".to_string(),
            arguments: vec![],
        };

        let result = expr.evaluate(context).await.unwrap();
        match result {
            ExprResult::String(s) => assert_eq!(s, "Hello, World!"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_unknown_static_function() {
        let expr = CallExpr {
            function: "func".to_string(),
            arguments: vec![],
        };

        let runtime = Rc::new(Runtime::new());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown function: func"));
    }

    #[tokio::test]
    async fn test_placeholder_parameter_population() {
        use crate::expressions::{InjectionExpr, PlaceholderExpr};

        let mut runtime = Runtime::new();

        let function_info = FunctionExpr {
            name: "process".to_string(),
            parameters: vec![("data".to_string(), Type::string())],
            return_type: Type::string(),
            body: vec![Box::new(InjectionExpr {
                inner: Box::new(StringLiteralExpr {
                    value: "Processing:".to_string(),
                }),
            })],
            documentation: None,
        };
        runtime.register_function(function_info);

        let runtime = Rc::new(runtime);
        let context = Arc::new(Context::with_runtime(runtime));

        context.add_event("Please provide data for processing".to_string());

        let expr = CallExpr {
            function: "process".to_string(),
            arguments: vec![Box::new(PlaceholderExpr {})],
        };

        let result = expr.evaluate(context.clone()).await.unwrap();

        match result {
            ExprResult::String(s) => {
                assert!(!s.is_empty());
            }
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events_count(), 1);
        assert_eq!(
            context.get_event(0).unwrap().message,
            "Please provide data for processing"
        );
    }

    #[tokio::test]
    async fn test_placeholder_with_multiple_context_events() {
        use crate::expressions::PlaceholderExpr;

        let mut runtime = Runtime::new();

        let function_info = FunctionExpr {
            name: "analyze".to_string(),
            parameters: vec![("comments".to_string(), Type::string())],
            return_type: Type::string(),
            body: vec![],
            documentation: None,
        };
        runtime.register_function(function_info);

        let runtime = Rc::new(runtime);
        let context = Arc::new(Context::with_runtime(runtime));

        context.add_event("Analyze the following".to_string());
        context.add_event("Focus on code quality".to_string());
        context.add_event("Provide actionable feedback".to_string());

        let expr = CallExpr {
            function: "analyze".to_string(),
            arguments: vec![Box::new(PlaceholderExpr {})],
        };

        let result = expr.evaluate(context.clone()).await.unwrap();

        assert_eq!(result, ExprResult::String("PrintEngine {}".to_string()));

        assert_eq!(context.events_count(), 3);
        assert_eq!(
            context.get_event(0).unwrap().message,
            "Analyze the following"
        );
        assert_eq!(
            context.get_event(1).unwrap().message,
            "Focus on code quality"
        );
        assert_eq!(
            context.get_event(2).unwrap().message,
            "Provide actionable feedback"
        );
    }
}
