use async_trait::async_trait;
use std::any::Any;
use std::sync::{Arc, LazyLock};
use structured_agent_runtime::{
    ExpressionResult, ExpressionValue, ExternalFunctionDefinition, Parameter, RuntimeError, Type,
};

use crate::context::{ActionEvent, Context, ThinkingEvent};

pub trait Event: Send + Sync {
    fn format(&self) -> String;
    fn return_type(&self) -> &Type;
}

static ACTION_EVENT_RETURN_TYPE: LazyLock<Type> = LazyLock::new(Type::string);

impl Event for ActionEvent {
    fn format(&self) -> String {
        let content = self.content.format_for_llm();
        if let Some(name) = &self.name {
            let params_xml = if let Some(params) = &self.params {
                let params_str = params
                    .iter()
                    .map(|p| {
                        let value = p.value.format_for_llm();
                        format!("    <param name=\"{}\">{}</param>", p.name, value)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{}\n", params_str)
            } else {
                String::new()
            };
            format!(
                "<{}>\n{}    <result>\n    {}\n    </result>\n</{}>",
                name, params_xml, content, name
            )
        } else {
            content
        }
    }

    fn return_type(&self) -> &Type {
        &ACTION_EVENT_RETURN_TYPE
    }
}

pub struct TypedEvent {
    pub return_type: Type,
}

impl Event for TypedEvent {
    fn format(&self) -> String {
        String::new()
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }
}

pub struct SelectEvent {
    pub options: Vec<ExpressionValue>,
}

static SELECT_EVENT_RETURN_TYPE: LazyLock<Type> = LazyLock::new(Type::int);

impl Event for SelectEvent {
    fn format(&self) -> String {
        let mut selection_prompt = "SELECT: Choose one of the following options:\n".to_string();
        for (index, option) in self.options.iter().enumerate() {
            let description = option
                .as_metadata()
                .ok()
                .map(|(name, doc)| match doc {
                    Some(d) => format!("Action: '{}' - {}", name, d),
                    None => format!("Action: '{}'", name),
                })
                .unwrap_or_else(|| option.format_for_llm());
            selection_prompt.push_str(&format!("{}: {}\n", index, description));
        }
        selection_prompt
    }

    fn return_type(&self) -> &Type {
        &SELECT_EVENT_RETURN_TYPE
    }
}

pub struct FillParameterEvent {
    pub function_name: String,
    pub param_name: String,
    pub param_type: Type,
}

impl Event for FillParameterEvent {
    fn format(&self) -> String {
        format!(
            "Return only the value for parameter '{}' of '{}'. Do not include any explanation or surrounding text.",
            self.param_name, self.function_name
        )
    }

    fn return_type(&self) -> &Type {
        &self.param_type
    }
}

#[async_trait]
pub trait Function: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn parameters(&self) -> &[Parameter];
    fn function_return_type(&self) -> &Type;
    async fn execute(
        &self,
        context: Context,
        args: Vec<ExpressionResult>,
    ) -> Result<(Context, ExpressionResult), String>;
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Function>;
    fn documentation(&self) -> Option<&str> {
        None
    }
}

#[async_trait]
pub trait ExecutableFunction: Function + std::fmt::Debug + Send + Sync {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction>;
}

#[async_trait]
pub trait LanguageEngine: Send + Sync {
    async fn request(
        &self,
        context: &Context,
        request: &dyn Event,
    ) -> Result<(ExpressionValue, Option<ThinkingEvent>), String>;
}

#[async_trait]
pub trait FunctionProvider: Send + Sync {
    async fn list_functions(&self) -> Result<Vec<ExternalFunctionDefinition>, RuntimeError>;
    async fn create_expression(
        &self,
        definition: &ExternalFunctionDefinition,
    ) -> Result<Arc<dyn ExecutableFunction>, RuntimeError>;
}

pub struct PrintEngine {}

#[async_trait]
impl LanguageEngine for PrintEngine {
    async fn request(
        &self,
        context: &Context,
        request: &dyn Event,
    ) -> Result<(ExpressionValue, Option<ThinkingEvent>), String> {
        let return_type = request.return_type();
        if return_type.is_unit() {
            return Ok((ExpressionValue::unit(), None));
        }
        if return_type.is_string() {
            let formatted = request.format();
            if !formatted.is_empty() {
                return Ok((ExpressionValue::string(formatted), None));
            }
            let value = context.last_event().map(|e| e.format()).unwrap_or_default();
            return Ok((ExpressionValue::string(value), None));
        }
        if return_type.is_boolean() {
            return Ok((ExpressionValue::boolean(true), None));
        }
        if return_type.is_int() {
            return Ok((ExpressionValue::integer(0), None));
        }
        match return_type {
            Type::Parameterized(n, args) => {
                if n.last_name() == "Option" {
                    Ok((
                        ExpressionValue::option_none_with_type(
                            context.runtime().type_to_arrow_datatype(&args[0]),
                        ),
                        None,
                    ))
                } else {
                    Ok((ExpressionValue::unit(), None))
                }
            }
            _ => Ok((ExpressionValue::unit(), None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::service::RuntimeService;
    use arrow::datatypes::DataType;
    use std::sync::Arc;
    use structured_agent_runtime::{DefinitionPath, ExpressionValue, Type};

    struct MockRuntime {
        engine: Arc<PrintEngine>,
    }

    impl RuntimeService for MockRuntime {
        fn get_native_function(&self, _name: &str) -> Option<Arc<dyn ExecutableFunction>> {
            None
        }

        fn get_bytecode_ref(
            &self,
            _name: &DefinitionPath,
        ) -> Option<structured_agent_il::BytecodeRef> {
            None
        }

        fn engine(&self) -> &dyn LanguageEngine {
            self.engine.as_ref()
        }

        fn type_to_arrow_datatype(&self, _ty: &Type) -> DataType {
            DataType::Utf8
        }

        fn get_struct(&self, _type_name: &DefinitionPath) -> Option<Vec<(String, Type)>> {
            None
        }

        fn get_struct_with_args(
            &self,
            _type_name: &DefinitionPath,
            _args: &[Type],
        ) -> Option<Vec<(String, Type)>> {
            None
        }
    }

    fn make_context() -> Context {
        let runtime = Arc::new(MockRuntime {
            engine: Arc::new(PrintEngine {}),
        });
        Context::with_runtime(runtime)
    }

    #[test]
    fn typed_event_return_type_matches() {
        let event = TypedEvent {
            return_type: Type::string(),
        };
        assert!(event.return_type().is_string());
    }

    #[test]
    fn typed_event_format_is_empty() {
        let event = TypedEvent {
            return_type: Type::string(),
        };
        assert_eq!(event.format(), "");
    }

    #[test]
    fn select_event_format_contains_options() {
        let event = SelectEvent {
            options: vec![
                ExpressionValue::metadata("Red", None),
                ExpressionValue::metadata("Blue", None),
            ],
        };
        let formatted = event.format();
        assert!(formatted.contains("Red"));
        assert!(formatted.contains("Blue"));
    }

    #[test]
    fn select_event_return_type_is_int() {
        let event = SelectEvent { options: vec![] };
        assert!(event.return_type().is_int());
    }

    #[test]
    fn fill_parameter_event_format_contains_name() {
        let event = FillParameterEvent {
            function_name: "some_fn".to_string(),
            param_name: "age".to_string(),
            param_type: Type::int(),
        };
        let formatted = event.format();
        assert!(formatted.contains("age"));
        assert!(formatted.contains("some_fn"));
        assert!(!formatted.contains("Int"));
    }

    #[test]
    fn fill_parameter_event_return_type_matches() {
        let event = FillParameterEvent {
            function_name: "some_fn".to_string(),
            param_name: "score".to_string(),
            param_type: Type::int(),
        };
        assert!(event.return_type().is_int());
    }

    #[test]
    fn action_event_format_unnamed() {
        let event = ActionEvent {
            content: ExpressionValue::string("hello".to_string()),
            name: None,
            params: None,
        };
        assert_eq!(event.format(), "hello");
    }

    #[test]
    fn action_event_format_named() {
        let event = ActionEvent {
            content: ExpressionValue::string("result".to_string()),
            name: Some("test".to_string()),
            params: None,
        };
        let formatted = event.format();
        assert!(formatted.contains("<test>"));
        assert!(formatted.contains("result"));
        assert!(formatted.contains("</test>"));
    }

    #[tokio::test]
    async fn print_engine_request_typed_string() {
        let engine = PrintEngine {};
        let mut context = make_context();
        context.add_event(ExpressionValue::string("hello".to_string()), None, None);
        let request = TypedEvent {
            return_type: Type::string(),
        };
        let result = engine.request(&context, &request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.type_name(), "String");
    }

    #[tokio::test]
    async fn print_engine_request_typed_bool() {
        let engine = PrintEngine {};
        let context = make_context();
        let request = TypedEvent {
            return_type: Type::boolean(),
        };
        let result = engine.request(&context, &request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.type_name(), "Boolean");
    }

    #[tokio::test]
    async fn print_engine_request_select() {
        let engine = PrintEngine {};
        let context = make_context();
        let request = SelectEvent {
            options: vec![
                ExpressionValue::metadata("Option A", None),
                ExpressionValue::metadata("Option B", None),
            ],
        };
        let result = engine.request(&context, &request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.type_name(), "Int");
    }

    #[tokio::test]
    async fn print_engine_request_fill_parameter() {
        let engine = PrintEngine {};
        let context = make_context();
        let request = FillParameterEvent {
            function_name: "some_fn".to_string(),
            param_name: "username".to_string(),
            param_type: Type::string(),
        };
        let result = engine.request(&context, &request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.type_name(), "String");
    }
}
