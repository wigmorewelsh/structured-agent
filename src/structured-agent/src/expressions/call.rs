use crate::expressions::PlaceholderExpr;
use crate::runtime::{Context, ExpressionParameter, ExpressionResult, ExpressionValue};
#[cfg(test)]
use crate::types::Parameter;
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use tracing::{Level, debug, info, span};

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
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let span = span!(
            Level::DEBUG,
            "function_call",
            function = %self.function,
            arg_count = self.arguments.len()
        );
        let _enter = span.enter();

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
                let param = &parameters[i];
                let param_name = &param.name;
                let param_type = &param.param_type;

                let value = context
                    .runtime()
                    .engine()
                    .fill_parameter(&context, param_name, param_type)
                    .await?;

                debug!(
                    function = %self.function,
                    parameter = %param_name,
                    param_type = %param_type.name(),
                    "placeholder_filled"
                );

                args.push(ExpressionResult::new(value));
            } else {
                args.push(arg.evaluate(context.clone()).await?);
            }
        }

        let function_context = Arc::new(Context::create_child(
            context.clone(),
            true,
            context.runtime_rc(),
        ));

        function_context.add_event(
            ExpressionValue::String(format!("## {}", self.function)),
            None,
            None,
        );

        let mut evaluated_parameters = Vec::new();

        for (i, param) in parameters.iter().enumerate() {
            let param_name = &param.name;
            function_context.declare_variable(param_name.clone(), args[i].clone());
            evaluated_parameters.push(ExpressionParameter::new(
                param_name.clone(),
                args[i].value.clone(),
            ));
        }

        let function_info = context
            .runtime()
            .get_function(&self.function)
            .expect("Function not found");

        let result = function_info.evaluate(function_context).await?;

        let result_display = match &result.value {
            ExpressionValue::String(s) => s.clone(),
            ExpressionValue::Boolean(b) => b.to_string(),
            ExpressionValue::Unit => "()".to_string(),
            _ => format!("{:?}", result.value),
        };

        info!(
            "<result function=\"{}\">\n{}\n</result>",
            self.function, result_display
        );

        debug!(
            function = %self.function,
            result_type = %result.value.type_name(),
            result_value = ?result.value,
            "function_result"
        );

        Ok(ExpressionResult {
            params: Some(evaluated_parameters),
            value: result.value,
        })
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

    fn name(&self) -> Option<&str> {
        Some(&self.function)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompilationUnit;
    use crate::expressions::{FunctionExpr, StringLiteralExpr};
    use crate::runtime::Runtime;
    use crate::types::{Parameter, Type};
    use std::rc::Rc;

    fn test_runtime() -> Runtime {
        let program = CompilationUnit::from_string("fn main(): () {}".to_string());
        Runtime::builder(program).build()
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let expr = CallExpr {
            function: "unknown_method".to_string(),
            arguments: vec![],
        };

        let runtime = Rc::new(test_runtime());
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
        let mut runtime = test_runtime();

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
        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "## hello"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_unknown_static_function() {
        let expr = CallExpr {
            function: "func".to_string(),
            arguments: vec![],
        };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = expr.evaluate(context).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown function: func"));
    }

    #[tokio::test]
    async fn test_placeholder_parameter_population() {
        use crate::expressions::{InjectionExpr, PlaceholderExpr};

        let mut runtime = test_runtime();

        let function_info = FunctionExpr {
            name: "process".to_string(),
            parameters: vec![Parameter::new("data".to_string(), Type::string())],
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

        context.add_event(
            ExpressionValue::String("Please provide data for processing".to_string()),
            None,
            None,
        );

        let expr = CallExpr {
            function: "process".to_string(),
            arguments: vec![Box::new(PlaceholderExpr {})],
        };

        let result = expr.evaluate(context.clone()).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => {
                assert!(!s.is_empty());
            }
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events_count(), 1);
        let event = context.get_event(0).unwrap();
        match event.content {
            ExpressionValue::String(s) => {
                assert_eq!(s, "Please provide data for processing")
            }
            _ => panic!("Expected string content in event"),
        }
    }

    #[tokio::test]
    async fn test_placeholder_with_multiple_context_events() {
        use crate::expressions::PlaceholderExpr;

        let mut runtime = test_runtime();

        let function_info = FunctionExpr {
            name: "analyze".to_string(),
            parameters: vec![Parameter::new("comments".to_string(), Type::string())],
            return_type: Type::string(),
            body: vec![],
            documentation: None,
        };
        runtime.register_function(function_info);

        let runtime = Rc::new(runtime);
        let context = Arc::new(Context::with_runtime(runtime));

        context.add_event(
            ExpressionValue::String("Analyze the following".to_string()),
            None,
            None,
        );
        context.add_event(
            ExpressionValue::String("Focus on code quality".to_string()),
            None,
            None,
        );
        context.add_event(
            ExpressionValue::String("Provide actionable feedback".to_string()),
            None,
            None,
        );

        let expr = CallExpr {
            function: "analyze".to_string(),
            arguments: vec![Box::new(PlaceholderExpr {})],
        };

        let result = expr.evaluate(context.clone()).await.unwrap();

        assert_eq!(
            result.value,
            ExpressionValue::String("## analyze".to_string())
        );

        assert_eq!(context.events_count(), 3);
        match &context.get_event(0).unwrap().content {
            ExpressionValue::String(s) => assert_eq!(s, "Analyze the following"),
            _ => panic!("Expected string content"),
        }
        match &context.get_event(1).unwrap().content {
            ExpressionValue::String(s) => assert_eq!(s, "Focus on code quality"),
            _ => panic!("Expected string content"),
        }
        match &context.get_event(2).unwrap().content {
            ExpressionValue::String(s) => assert_eq!(s, "Provide actionable feedback"),
            _ => panic!("Expected string content"),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_tracing_instrumentation() {
        use std::sync::{Arc as StdArc, Mutex};
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

        #[derive(Clone)]
        struct TestLayer {
            events: StdArc<Mutex<Vec<String>>>,
        }

        impl<S> tracing_subscriber::Layer<S> for TestLayer
        where
            S: tracing::Subscriber,
        {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _ctx: tracing_subscriber::layer::Context<'_, S>,
            ) {
                let mut visitor = EventVisitor {
                    message: String::new(),
                };
                event.record(&mut visitor);
                self.events.lock().unwrap().push(visitor.message);
            }
        }

        struct EventVisitor {
            message: String,
        }

        impl EventVisitor {
            fn add_field(&mut self, name: &str, value: String) {
                if name == "message" {
                    self.message = value;
                } else {
                    if !self.message.is_empty() {
                        self.message.push_str(", ");
                    }
                    self.message.push_str(&format!("{}={}", name, value));
                }
            }
        }

        impl tracing::field::Visit for EventVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.add_field(field.name(), format!("{:?}", value));
            }

            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.add_field(field.name(), value.to_string());
            }
        }

        let events = StdArc::new(Mutex::new(Vec::new()));
        let test_layer = TestLayer {
            events: events.clone(),
        };

        let subscriber = tracing_subscriber::registry().with(test_layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut runtime = test_runtime();
        let function_info = FunctionExpr {
            name: "test_func".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            body: vec![Box::new(StringLiteralExpr {
                value: "result".to_string(),
            })],
            documentation: None,
        };
        runtime.register_function(function_info);

        let runtime = Rc::new(runtime);
        let context = Arc::new(Context::with_runtime(runtime));

        let expr = CallExpr {
            function: "test_func".to_string(),
            arguments: vec![],
        };

        let _result = expr.evaluate(context).await.unwrap();

        let recorded_events = events.lock().unwrap();
        assert!(
            recorded_events
                .iter()
                .any(|e| e.contains("function_result"))
        );
    }
}
