use crate::error::GeminiResult;
use crate::types::GenerationConfig;
use crate::types::JsonSchemaBuilder;
use crate::{ChatMessage, GeminiClient, GeminiConfig, ModelName};
use async_trait::async_trait;
use schemars::schema::SchemaObject;

use structured_agent_interpreter_runtime::{
    Context, ContextEvent, Event, ExpressionValue, LanguageEngine, ThinkingEvent, Type,
};

const DEFAULT_NO_EVENTS_MESSAGE: &str = "No events available.";
const DEFAULT_NO_RESPONSE_MESSAGE: &str = "No response received";

pub struct GeminiEngine {
    client: GeminiClient,
    model: ModelName,
}

impl GeminiEngine {
    pub async fn new(config: GeminiConfig) -> GeminiResult<Self> {
        let client = GeminiClient::new(config).await?;

        Ok(Self {
            client,
            model: ModelName::default(),
        })
    }

    pub async fn from_env() -> GeminiResult<Self> {
        let client = GeminiClient::from_env().await?;

        Ok(Self {
            client,
            model: ModelName::default(),
        })
    }

    pub fn with_model(mut self, model: ModelName) -> Self {
        self.model = model;
        self
    }

    fn build_value_schema(value_type: &Type, context: &Context) -> Result<SchemaObject, String> {
        match value_type {
            _ if value_type.is_string() => Ok(JsonSchemaBuilder::string()),
            _ if value_type.is_boolean() => Ok(JsonSchemaBuilder::boolean()),
            _ if value_type.is_int() => Ok(JsonSchemaBuilder::integer()),
            Type::Parameterized(n, _) if n.last_name() == "List" => {
                Ok(JsonSchemaBuilder::array(JsonSchemaBuilder::string()))
            }
            Type::Parameterized(n, args) if n.last_name() == "Option" => {
                Self::build_value_schema(&args[0], context)
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
                let mut obj = JsonSchemaBuilder::object();
                for (field_name, field_type) in &fields {
                    let field_schema = Self::build_value_schema(field_type, context)?;
                    obj = JsonSchemaBuilder::with_property(obj, field_name, field_schema, true);
                }
                Ok(obj)
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
                let mut obj = JsonSchemaBuilder::object();
                for (field_name, field_type) in &fields {
                    let field_schema = Self::build_value_schema(field_type, context)?;
                    obj = JsonSchemaBuilder::with_property(obj, field_name, field_schema, true);
                }
                Ok(obj)
            }
        }
    }

    fn build_context_messages(&self, context: &Context) -> Vec<ChatMessage> {
        let all_events: Vec<ContextEvent> = context.iter_all_context_events().collect();

        if all_events.is_empty() {
            return vec![ChatMessage::system(DEFAULT_NO_EVENTS_MESSAGE)];
        }

        all_events
            .iter()
            .map(|event| match event {
                ContextEvent::Action(a) => ChatMessage::system(a.format()),
                ContextEvent::Thinking(t) => {
                    ChatMessage::thinking_model(t.content.clone(), t.thought_signature.clone())
                }
            })
            .collect()
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
}

#[async_trait]
impl LanguageEngine for GeminiEngine {
    async fn request(
        &self,
        context: &Context,
        request: &dyn Event,
    ) -> Result<(ExpressionValue, Option<ThinkingEvent>), String> {
        let return_type = request.return_type();

        if return_type.is_unit() {
            return Ok((ExpressionValue::unit(), None));
        }

        let value_schema = Self::build_value_schema(return_type, context)?;
        let is_required = !return_type.is_option();
        let temperature = if return_type.is_boolean() { 0.0 } else { 0.7 };

        let schema = JsonSchemaBuilder::with_property(
            JsonSchemaBuilder::object(),
            "value",
            value_schema,
            is_required,
        );

        let mut chat_messages = self.build_context_messages(context);
        let user_message = request.format();
        if !user_message.is_empty() {
            chat_messages.push(ChatMessage::user(user_message));
        }

        let generation_config = GenerationConfig::new()
            .with_temperature(temperature)
            .with_top_p(0.95)
            .with_response_mime_type("application/json".to_string())
            .with_response_schema(schema)
            .with_minimal_thinking();

        let response = self
            .client
            .structured_chat(chat_messages, self.model.clone(), Some(generation_config))
            .await
            .map_err(|e| format!("Error communicating with Gemini: {}", e))?;

        let thinking = response
            .first_thinking()
            .map(|(content, signature)| ThinkingEvent {
                content,
                thought_signature: signature,
            });

        let response_text = response
            .first_content()
            .unwrap_or_else(|| DEFAULT_NO_RESPONSE_MESSAGE.to_string());

        Self::parse_typed_response(&response_text, return_type, context)
            .map(|value| (value, thinking))
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
        fn get_bytecode_ref(
            &self,
            _: &DefinitionPath,
        ) -> Option<structured_agent_interpreter_runtime::BytecodeRef> {
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
        let result = GeminiEngine::build_value_schema(&ghost_type, &context);
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
        let result = GeminiEngine::build_value_schema(&task_type, &context);
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
        let result = GeminiEngine::parse_json_value(json, &point_type, &context);
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
        let result = GeminiEngine::build_value_schema(&task_type, &context);
        assert!(result.is_ok(), "Expected schema, got: {:?}", result.err());
    }

    #[test]
    fn test_build_value_schema_parameterized_unknown_returns_error() {
        let context = empty_context();
        let ghost_type = Type::Parameterized(make_def_path("test", "Ghost"), vec![]);
        let result = GeminiEngine::build_value_schema(&ghost_type, &context);
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
        let result = GeminiEngine::parse_json_value(json, &point_type, &context);
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
