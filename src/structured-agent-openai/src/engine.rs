use async_openai::{
    Client,
    config::{Config, OpenAIConfig},
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPartAudio,
        ChatCompletionRequestMessageContentPartImage, ChatCompletionRequestMessageContentPartText,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestUserMessageContentPart, CreateChatCompletionRequestArgs, ImageUrl,
        InputAudio, InputAudioFormat, ReasoningEffort, ResponseFormat, ResponseFormatJsonSchema,
    },
};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use structured_agent_interpreter_runtime::{
    Context, ContextEvent, DefinitionPath, Event, ExpressionValue, LanguageEngine, ThinkingEvent,
    Type,
};

const DEFAULT_NO_EVENTS_MESSAGE: &str = "No events available.";
const DEFAULT_NO_RESPONSE_MESSAGE: &str = "No response received";
pub const HF_BASE_URL: &str = "https://api-inference.huggingface.co/v1";
pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAIEngine<C: Config = OpenAIConfig> {
    pub(crate) client: Client<C>,
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
}

impl OpenAIEngine<OpenAIConfig> {
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
            reasoning_effort: None,
        }
    }

    pub fn with_reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }
}

impl<C: Config> OpenAIEngine<C> {
    pub(crate) fn with_client(client: Client<C>, model: impl Into<String>) -> Self {
        Self {
            client,
            model: model.into(),
            reasoning_effort: None,
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
            Type::Named(tn) if tn.last_name() == "Unit" => {
                Err("Unit type cannot be used in schema".to_string())
            }
            Type::Generic(name) => Err(format!("Generic type {} cannot be used in schema", name)),
            Type::Union(variants) => Err(format!(
                "Union type {} cannot be used in schema",
                variants
                    .iter()
                    .map(|v| v.name())
                    .collect::<Vec<_>>()
                    .join(" | ")
            )),
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
        let all_events: Vec<ContextEvent> = context.iter_all_context_events().collect();
        if all_events.is_empty() {
            return vec![
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(DEFAULT_NO_EVENTS_MESSAGE)
                    .build()
                    .unwrap()
                    .into(),
            ];
        }
        all_events
            .iter()
            .filter_map(|event| match event {
                ContextEvent::Action(a) => {
                    if let Some(media_part) = Self::expression_value_to_content_part(&a.content) {
                        let text_part = ChatCompletionRequestUserMessageContentPart::Text(
                            ChatCompletionRequestMessageContentPartText { text: a.format() },
                        );
                        Some(
                            ChatCompletionRequestUserMessageArgs::default()
                                .content(vec![text_part, media_part])
                                .build()
                                .unwrap()
                                .into(),
                        )
                    } else {
                        Some(
                            ChatCompletionRequestSystemMessageArgs::default()
                                .content(a.format())
                                .build()
                                .unwrap()
                                .into(),
                        )
                    }
                }
                ContextEvent::Thinking(_) => None,
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
            Type::Union(variants) if variants.contains(&Type::unit()) => {
                if json_value.is_null() {
                    Ok(ExpressionValue::unit())
                } else {
                    let inner = variants
                        .iter()
                        .find(|v| !v.is_unit())
                        .map(|v| v.clone())
                        .unwrap_or_else(Type::string);
                    Self::parse_json_value(json_value, &inner, context)
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
                Self::parse_struct_fields(obj, type_name, &fields, context)
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
                Self::parse_struct_fields(obj, n, &fields, context)
            }
            _ => Err(format!("Unsupported type: {}", value_type.name())),
        }
    }

    fn parse_struct_fields(
        obj: &serde_json::Map<String, serde_json::Value>,
        type_name: &DefinitionPath,
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
        Ok(ExpressionValue::named_struct_value(
            type_name.clone(),
            field_values,
        ))
    }

    fn parse_typed_response(
        response_text: &str,
        return_type: &Type,
        context: &Context,
    ) -> Result<ExpressionValue, String> {
        let response_json: serde_json::Value = serde_json::from_str(response_text)
            .map_err(|_| format!("Invalid JSON response: '{}'", response_text))?;

        match return_type {
            _ if return_type.is_unit() => {
                Err("Unit type cannot be used as return type".to_string())
            }
            Type::Generic(_) => Err("Generic type cannot be used as return type".to_string()),
            Type::Union(_) => Err("Union type cannot be used as return type".to_string()),
            _ => {
                let value_field = response_json
                    .get("value")
                    .ok_or_else(|| "Missing 'value' field in response".to_string())?;
                Self::parse_json_value(value_field.clone(), return_type, context)
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

    pub(crate) fn expression_value_to_content_part(
        value: &ExpressionValue,
    ) -> Option<ChatCompletionRequestUserMessageContentPart> {
        if let Ok(img) = value.as_image() {
            let url = format!(
                "data:{};base64,{}",
                img.mime_type,
                STANDARD.encode(&img.data)
            );
            return Some(ChatCompletionRequestUserMessageContentPart::ImageUrl(
                ChatCompletionRequestMessageContentPartImage {
                    image_url: ImageUrl { url, detail: None },
                },
            ));
        }
        if let Ok(aud) = value.as_audio() {
            let format = match aud.mime_type.split('/').nth(1).unwrap_or("") {
                "wav" => InputAudioFormat::Wav,
                _ => InputAudioFormat::Mp3,
            };
            return Some(ChatCompletionRequestUserMessageContentPart::InputAudio(
                ChatCompletionRequestMessageContentPartAudio {
                    input_audio: InputAudio {
                        data: STANDARD.encode(&aud.data),
                        format,
                    },
                },
            ));
        }
        if let Ok(lnk) = value.as_link() {
            return Some(ChatCompletionRequestUserMessageContentPart::Text(
                ChatCompletionRequestMessageContentPartText { text: lnk.uri },
            ));
        }
        None
    }
}

#[async_trait]
impl<C: Config + Send + Sync + 'static> LanguageEngine for OpenAIEngine<C> {
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
        let is_required = !return_type
            .union_variants()
            .map_or(false, |vs| vs.contains(&Type::unit()));
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

        let mut request_builder = CreateChatCompletionRequestArgs::default();
        request_builder
            .model(&self.model)
            .messages(messages)
            .response_format(Self::make_json_schema_format(schema))
            .temperature(temperature);
        if let Some(effort) = &self.reasoning_effort {
            request_builder.reasoning_effort(effort.clone());
        }
        let openai_request = request_builder
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

        Self::parse_typed_response(&response_text, return_type, context).map(|v| (v, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type OpenAIEngine = super::OpenAIEngine<OpenAIConfig>;
    use arrow::datatypes::DataType;
    use async_openai::types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessageContent,
        ChatCompletionRequestUserMessageContentPart, InputAudioFormat,
    };
    use base64::engine::general_purpose::STANDARD;
    use nonempty::NonEmpty;
    use std::collections::HashMap;
    use std::sync::Arc;
    use structured_agent_interpreter_runtime::{
        DefinitionPath, ExecutableFunction, RuntimeService, Source,
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

    #[test]
    fn test_parse_typed_response_struct() {
        let mut structs = HashMap::new();
        structs.insert(
            "Done".to_string(),
            vec![("report".to_string(), Type::string())],
        );
        let context = make_context(structs);
        let response = "{\"value\": {\"report\": \"all done\"}}";
        let done_type = Type::Named(make_def_path("main", "Done"));
        let result = OpenAIEngine::parse_typed_response(response, &done_type, &context);
        assert!(result.is_ok(), "Expected value, got: {:?}", result.err());
        assert_eq!(
            result
                .unwrap()
                .get_struct_field("report")
                .unwrap()
                .as_string()
                .unwrap(),
            "all done"
        );
    }

    #[test]
    fn test_parse_typed_response_struct_has_correct_type_path() {
        let mut structs = HashMap::new();
        structs.insert(
            "Done".to_string(),
            vec![("report".to_string(), Type::string())],
        );
        let context = make_context(structs);
        let response = "{\"value\": {\"report\": \"all done\"}}";
        let done_type = Type::Named(make_def_path("main", "Done"));
        let result = OpenAIEngine::parse_typed_response(response, &done_type, &context).unwrap();
        assert_eq!(result.type_path, make_def_path("main", "Done"));
    }

    #[test]
    fn image_expression_value_maps_to_openai_image_part() {
        let value = ExpressionValue::image("image/png", vec![1u8, 2, 3]);
        let part = OpenAIEngine::expression_value_to_content_part(&value).unwrap();
        let ChatCompletionRequestUserMessageContentPart::ImageUrl(image_part) = part else {
            panic!("Expected ImageUrl variant");
        };
        assert_eq!(image_part.image_url.url, "data:image/png;base64,AQID");
        assert!(image_part.image_url.detail.is_none());
    }

    #[test]
    fn audio_expression_value_maps_to_openai_audio_part() {
        let value = ExpressionValue::audio("audio/mp3", vec![4u8, 5, 6]);
        let part = OpenAIEngine::expression_value_to_content_part(&value).unwrap();
        let ChatCompletionRequestUserMessageContentPart::InputAudio(audio_part) = part else {
            panic!("Expected InputAudio variant");
        };
        assert_eq!(audio_part.input_audio.data, STANDARD.encode([4u8, 5, 6]));
        assert_eq!(audio_part.input_audio.format, InputAudioFormat::Mp3);
    }

    #[test]
    fn build_context_messages_with_image_adds_image_content_part() {
        let mut context = empty_context();
        context.add_event(
            ExpressionValue::image("image/png", vec![1u8, 2, 3]),
            None,
            None,
            Source::System,
        );
        let messages = OpenAIEngine::build_context_messages(&context);
        assert_eq!(messages.len(), 1);
        let ChatCompletionRequestMessage::User(user_msg) = &messages[0] else {
            panic!("Expected user message");
        };
        let ChatCompletionRequestUserMessageContent::Array(parts) = &user_msg.content else {
            panic!("Expected array content");
        };
        assert!(
            parts
                .iter()
                .any(|p| matches!(p, ChatCompletionRequestUserMessageContentPart::ImageUrl(_)))
        );
    }

    #[test]
    fn build_context_messages_with_string_has_single_text_part() {
        let mut context = empty_context();
        context.add_event(
            ExpressionValue::string("hello".to_string()),
            None,
            None,
            Source::System,
        );
        let messages = OpenAIEngine::build_context_messages(&context);
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0],
            ChatCompletionRequestMessage::System(_)
        ));
    }

    #[test]
    fn link_expression_value_maps_to_openai_text_part() {
        let value = ExpressionValue::link("https://example.com", None);
        let part = OpenAIEngine::expression_value_to_content_part(&value).unwrap();
        let ChatCompletionRequestUserMessageContentPart::Text(text_part) = part else {
            panic!("Expected Text variant");
        };
        assert_eq!(text_part.text, "https://example.com");
    }
}
