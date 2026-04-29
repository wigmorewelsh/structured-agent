use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, ResponseFormat,
        ResponseFormatJsonSchema,
    },
};
use async_trait::async_trait;
use structured_agent_interpreter_runtime::{
    ActionEvent, Context, Event, ExpressionValue, LanguageEngine, Type,
};

const DEFAULT_NO_EVENTS_MESSAGE: &str = "No events available.";
const DEFAULT_NO_RESPONSE_MESSAGE: &str = "No response received";
pub const HF_BASE_URL: &str = "https://api-inference.huggingface.co/v1";
pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAIEngine {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAIEngine {
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let api_key = api_key.into();
        let base_url = base_url.into();
        let config = OpenAIConfig::new()
            .with_api_key(&api_key)
            .with_api_base(&base_url);
        Self {
            client: Client::with_config(config),
            model: model.into(),
        }
    }

    fn build_value_schema(
        value_type: &Type,
        context: &Context,
    ) -> Result<serde_json::Value, String> {
        match value_type {
            _ if value_type.is_string() => Ok(serde_json::json!({"type": "string"})),
            _ if value_type.is_boolean() => Ok(serde_json::json!({"type": "boolean"})),
            _ if value_type.is_int() => Ok(serde_json::json!({"type": "integer"})),
            Type::Parameterized(n, _) if n.last_name() == "List" => {
                Ok(serde_json::json!({"type": "array", "items": {"type": "string"}}))
            }
            Type::Parameterized(n, args) if n.last_name() == "Option" => {
                let inner = Self::build_value_schema(&args[0], context)?;
                Ok(serde_json::json!({"anyOf": [inner, {"type": "null"}]}))
            }
            Type::Named(tn) if tn.last_name() == "Unit" => {
                Err("Unit type cannot be used in schema".to_string())
            }
            Type::Generic(name) => Err(format!("Generic type {} cannot be used in schema", name)),
            Type::Named(type_name) => {
                let fields = context
                    .runtime()
                    .get_struct(type_name)
                    .ok_or_else(|| format!("Unknown struct: {}", type_name.last_name()))?;
                Self::build_object_schema(&fields, context)
            }
            Type::Parameterized(n, args) => {
                let fields = context
                    .runtime()
                    .get_struct_with_args(n, args)
                    .ok_or_else(|| {
                        format!(
                            "Parameterized type {} cannot be used in schema",
                            n.last_name()
                        )
                    })?;
                Self::build_object_schema(&fields, context)
            }
        }
    }

    fn build_object_schema(
        fields: &[(String, Type)],
        context: &Context,
    ) -> Result<serde_json::Value, String> {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for (field_name, field_type) in fields {
            properties.insert(
                field_name.clone(),
                Self::build_value_schema(field_type, context)?,
            );
            required.push(serde_json::Value::String(field_name.clone()));
        }
        Ok(serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }))
    }

    fn build_context_messages(context: &Context) -> Vec<ChatCompletionRequestMessage> {
        let events: Vec<ActionEvent> = context.iter_all_events().collect();
        if events.is_empty() {
            vec![
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(DEFAULT_NO_EVENTS_MESSAGE)
                    .build()
                    .unwrap()
                    .into(),
            ]
        } else {
            events
                .iter()
                .map(|event| {
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(event.format())
                        .build()
                        .unwrap()
                        .into()
                })
                .collect()
        }
    }

    fn parse_json_value(
        json_value: serde_json::Value,
        value_type: &Type,
        context: &Context,
    ) -> Result<ExpressionValue, String> {
        match value_type {
            _ if value_type.is_string() => {
                if let Some(s) = json_value.as_str() {
                    Ok(ExpressionValue::string(s))
                } else {
                    Err("Expected string value".to_string())
                }
            }
            _ if value_type.is_boolean() => {
                if let Some(b) = json_value.as_bool() {
                    Ok(ExpressionValue::boolean(b))
                } else {
                    Err("Expected boolean value".to_string())
                }
            }
            _ if value_type.is_int() => {
                if let Some(n) = json_value.as_i64() {
                    Ok(ExpressionValue::integer(n))
                } else {
                    Err("Expected integer value".to_string())
                }
            }
            Type::Parameterized(n, _) if n.last_name() == "List" => {
                let items: Vec<String> = if json_value.is_array() {
                    json_value
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                } else {
                    return Err("Expected array value".to_string());
                };
                let mut builder =
                    arrow::array::ListBuilder::new(arrow::array::StringBuilder::new());
                let values_builder = builder.values();
                for item in &items {
                    values_builder.append_value(item);
                }
                builder.append(true);
                Ok(ExpressionValue::list(std::sync::Arc::new(builder.finish())))
            }
            Type::Parameterized(n, args) if n.last_name() == "Option" => {
                if json_value.is_null() {
                    Ok(ExpressionValue::option_none_with_type(
                        context.runtime().type_to_arrow_datatype(&args[0]),
                    ))
                } else {
                    let inner = Self::parse_json_value(json_value, &args[0], context)?;
                    Ok(ExpressionValue::option_some(inner))
                }
            }
            Type::Named(type_name) => {
                let obj = json_value.as_object().ok_or_else(|| {
                    format!("Expected JSON object for struct {}", type_name.last_name())
                })?;
                let fields = context
                    .runtime()
                    .get_struct(type_name)
                    .ok_or_else(|| format!("Unknown struct: {}", type_name.last_name()))?
                    .clone();
                Self::parse_struct_fields(obj, &fields, context)
            }
            Type::Parameterized(n, args) => {
                let obj = json_value
                    .as_object()
                    .ok_or_else(|| format!("Expected JSON object for struct {}", n.last_name()))?;
                let fields = context
                    .runtime()
                    .get_struct_with_args(n, args)
                    .ok_or_else(|| format!("Unknown struct: {}", n.last_name()))?
                    .clone();
                Self::parse_struct_fields(obj, &fields, context)
            }
            _ => Err(format!("Unsupported type: {}", value_type.name())),
        }
    }

    fn parse_struct_fields(
        obj: &serde_json::Map<String, serde_json::Value>,
        fields: &[(String, Type)],
        context: &Context,
    ) -> Result<ExpressionValue, String> {
        let field_values: Vec<(&str, ExpressionValue)> = fields
            .iter()
            .map(|(field_name, field_type)| {
                let json_field = obj
                    .get(field_name)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let val = Self::parse_json_value(json_field, field_type, context)?;
                Ok((field_name.as_str(), val))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(ExpressionValue::struct_value(field_values))
    }

    fn parse_typed_response(
        response_text: &str,
        return_type: &Type,
        context: &Context,
    ) -> Result<ExpressionValue, String> {
        let response_json: serde_json::Value = serde_json::from_str(response_text)
            .map_err(|_| format!("Invalid JSON response: '{}'", response_text))?;

        match return_type {
            Type::Named(_) if !return_type.is_unit() => {
                Self::parse_json_value(response_json, return_type, context)
            }
            _ => {
                let value_field = response_json
                    .get("value")
                    .ok_or_else(|| "Missing 'value' field in response".to_string())?;
                match return_type {
                    _ if return_type.is_string()
                        || return_type.is_boolean()
                        || return_type.is_int() =>
                    {
                        Self::parse_json_value(value_field.clone(), return_type, context)
                    }
                    Type::Parameterized(_, _) => {
                        Self::parse_json_value(value_field.clone(), return_type, context)
                    }
                    _ if return_type.is_unit() => {
                        Err("Unit type cannot be used as return type".to_string())
                    }
                    Type::Generic(_) => {
                        Err("Generic type cannot be used as return type".to_string())
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    fn make_json_schema_format(schema: serde_json::Value) -> ResponseFormat {
        ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                description: None,
                name: "response".to_string(),
                schema: Some(schema),
                strict: Some(false),
            },
        }
    }
}

#[async_trait]
impl LanguageEngine for OpenAIEngine {
    async fn request(
        &self,
        context: &Context,
        request: &dyn Event,
    ) -> Result<ExpressionValue, String> {
        let return_type = request.return_type();

        if return_type.is_unit() {
            return Ok(ExpressionValue::unit());
        }

        let value_schema = Self::build_value_schema(return_type, context)?;
        let is_required = !return_type.is_option();
        let temperature = if return_type.is_boolean() {
            0.0f32
        } else {
            0.7f32
        };

        let schema = if matches!(return_type, Type::Named(_)) && !return_type.is_unit() {
            value_schema
        } else {
            let mut properties = serde_json::Map::new();
            properties.insert("value".to_string(), value_schema);
            let required: Vec<serde_json::Value> = if is_required {
                vec![serde_json::Value::String("value".to_string())]
            } else {
                vec![]
            };
            serde_json::json!({
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false
            })
        };

        let mut messages = Self::build_context_messages(context);
        let prompt = request.format();
        if !prompt.is_empty() {
            messages.push(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(prompt)
                    .build()
                    .unwrap()
                    .into(),
            );
        }

        let openai_request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .response_format(Self::make_json_schema_format(schema))
            .temperature(temperature)
            .build()
            .map_err(|e| format!("Error building request: {}", e))?;

        let response = self
            .client
            .chat()
            .create(openai_request)
            .await
            .map_err(|e| format!("Error communicating with OpenAI: {}", e))?;

        let response_text = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| DEFAULT_NO_RESPONSE_MESSAGE.to_string());

        Self::parse_typed_response(&response_text, return_type, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::datatypes::DataType;
    use nonempty::NonEmpty;
    use std::collections::HashMap;
    use std::sync::Arc;
    use structured_agent_interpreter_runtime::{
        DefinitionPath, ExecutableFunction, RuntimeService,
    };

    struct MockRuntime {
        structs: HashMap<String, Vec<(String, Type)>>,
    }

    impl RuntimeService for MockRuntime {
        fn get_native_function(&self, _: &str) -> Option<Arc<dyn ExecutableFunction>> {
            None
        }
        fn get_bytecode_function(&self, _: &DefinitionPath) -> Option<Arc<dyn ExecutableFunction>> {
            None
        }
        fn engine(&self) -> &dyn LanguageEngine {
            unimplemented!()
        }
        fn type_to_arrow_datatype(&self, _: &Type) -> DataType {
            DataType::Null
        }
        fn get_struct(&self, type_name: &DefinitionPath) -> Option<Vec<(String, Type)>> {
            self.structs.get(type_name.last_name()).cloned()
        }
        fn get_struct_with_args(
            &self,
            type_name: &DefinitionPath,
            _: &[Type],
        ) -> Option<Vec<(String, Type)>> {
            self.structs.get(type_name.last_name()).cloned()
        }
    }

    fn make_context(structs: HashMap<String, Vec<(String, Type)>>) -> Context {
        Context::with_runtime(Arc::new(MockRuntime { structs }))
    }

    fn empty_context() -> Context {
        make_context(HashMap::new())
    }

    fn make_def_path(module: &str, type_name: &str) -> DefinitionPath {
        DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new(module.to_string())),
            type_name,
        )
    }

    #[test]
    fn test_build_value_schema_struct_unknown_returns_error() {
        let context = empty_context();
        let ghost_type = Type::Named(make_def_path("test", "Ghost"));
        let result = OpenAIEngine::build_value_schema(&ghost_type, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Ghost"));
    }

    #[test]
    fn test_build_value_schema_struct_with_fields() {
        let mut structs = HashMap::new();
        structs.insert(
            "Task".to_string(),
            vec![
                ("title".to_string(), Type::string()),
                ("steps".to_string(), Type::int()),
            ],
        );
        let context = make_context(structs);
        let task_type = Type::Named(make_def_path("main", "Task"));
        let result = OpenAIEngine::build_value_schema(&task_type, &context);
        assert!(result.is_ok(), "Expected schema, got: {:?}", result.err());
    }

    #[test]
    fn test_parse_json_value_struct() {
        let mut structs = HashMap::new();
        structs.insert(
            "Point".to_string(),
            vec![
                ("x".to_string(), Type::int()),
                ("y".to_string(), Type::int()),
            ],
        );
        let context = make_context(structs);
        let json = serde_json::json!({"x": 10, "y": 20});
        let point_type = Type::Named(make_def_path("main", "Point"));
        let result = OpenAIEngine::parse_json_value(json, &point_type, &context);
        assert!(result.is_ok(), "Expected value, got: {:?}", result.err());
        let value = result.unwrap();
        assert_eq!(
            value.get_struct_field("x").unwrap().as_integer().unwrap(),
            10
        );
        assert_eq!(
            value.get_struct_field("y").unwrap().as_integer().unwrap(),
            20
        );
    }

    #[test]
    fn test_build_value_schema_parameterized_known_struct() {
        let mut structs = HashMap::new();
        structs.insert(
            "Task".to_string(),
            vec![
                ("title".to_string(), Type::string()),
                ("steps".to_string(), Type::int()),
            ],
        );
        let context = make_context(structs);
        let task_type = Type::Parameterized(make_def_path("main", "Task"), vec![]);
        let result = OpenAIEngine::build_value_schema(&task_type, &context);
        assert!(result.is_ok(), "Expected schema, got: {:?}", result.err());
    }

    #[test]
    fn test_build_value_schema_parameterized_unknown_returns_error() {
        let context = empty_context();
        let ghost_type = Type::Parameterized(make_def_path("test", "Ghost"), vec![]);
        let result = OpenAIEngine::build_value_schema(&ghost_type, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Ghost"));
    }

    #[test]
    fn test_parse_json_value_parameterized_struct() {
        let mut structs = HashMap::new();
        structs.insert(
            "Point".to_string(),
            vec![
                ("x".to_string(), Type::int()),
                ("y".to_string(), Type::int()),
            ],
        );
        let context = make_context(structs);
        let json = serde_json::json!({"x": 3, "y": 7});
        let point_type = Type::Parameterized(make_def_path("main", "Point"), vec![]);
        let result = OpenAIEngine::parse_json_value(json, &point_type, &context);
        assert!(result.is_ok(), "Expected value, got: {:?}", result.err());
        let value = result.unwrap();
        assert_eq!(
            value.get_struct_field("x").unwrap().as_integer().unwrap(),
            3
        );
        assert_eq!(
            value.get_struct_field("y").unwrap().as_integer().unwrap(),
            7
        );
    }
}
