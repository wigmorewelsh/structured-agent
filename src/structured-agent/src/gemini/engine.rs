use crate::gemini::error::GeminiResult;
use crate::gemini::types::GenerationConfig;
use crate::gemini::types::JsonSchemaBuilder;
use crate::gemini::{ChatMessage, GeminiClient, GeminiConfig, ModelName};
use crate::runtime::Context;
use crate::runtime::Event;
use crate::runtime::ExpressionValue;
use crate::types::LanguageEngine;
use crate::types::Type;
use async_trait::async_trait;
use schemars::schema::SchemaObject;
use serde::{Deserialize, Serialize};

const DEFAULT_NO_EVENTS_MESSAGE: &str = "No events available.";
const DEFAULT_NO_RESPONSE_MESSAGE: &str = "No response received";

#[derive(Serialize, Deserialize)]
struct SelectionResponse {
    selection: u32,
}

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

    fn build_value_schema(value_type: &Type) -> Result<SchemaObject, String> {
        match value_type {
            Type::String => Ok(JsonSchemaBuilder::string()),
            Type::Boolean => Ok(JsonSchemaBuilder::boolean()),
            Type::List(_) => Ok(JsonSchemaBuilder::array(JsonSchemaBuilder::string())),
            Type::Option(inner_type) => Self::build_value_schema(inner_type),
            Type::Unit => Err("Unit type cannot be used in schema".to_string()),
            Type::Custom(_) => Err(format!("Unsupported type: {}", value_type.name())),
        }
    }

    fn format_event(event: &Event) -> String {
        let content = event.content.format_for_llm();

        if let Some(name) = &event.name {
            let params_xml = if let Some(params) = &event.params {
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

    fn build_context_messages(&self, context: &Context) -> Vec<ChatMessage> {
        let events: Vec<_> = context.iter_all_events().collect();

        if events.is_empty() {
            vec![ChatMessage::system(DEFAULT_NO_EVENTS_MESSAGE)]
        } else {
            events
                .iter()
                .map(|event| ChatMessage::system(&Self::format_event(event)))
                .collect()
        }
    }

    fn parse_json_value(
        json_value: serde_json::Value,
        value_type: &Type,
    ) -> Result<ExpressionValue, String> {
        match value_type {
            Type::String => {
                if let Some(s) = json_value.as_str() {
                    Ok(ExpressionValue::String(s.to_string()))
                } else {
                    Err("Expected string value".to_string())
                }
            }
            Type::Boolean => {
                if let Some(b) = json_value.as_bool() {
                    Ok(ExpressionValue::Boolean(b))
                } else {
                    Err("Expected boolean value".to_string())
                }
            }
            Type::List(_) => {
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
                Ok(ExpressionValue::List(std::sync::Arc::new(builder.finish())))
            }
            Type::Option(inner_type) => {
                if json_value.is_null() {
                    Ok(ExpressionValue::Option(None))
                } else {
                    let inner_result = Self::parse_json_value(json_value, inner_type)?;
                    Ok(ExpressionValue::Option(Some(Box::new(inner_result))))
                }
            }
            _ => Err(format!("Unsupported type: {}", value_type.name())),
        }
    }

    fn parse_typed_response(
        response_text: &str,
        return_type: &Type,
    ) -> Result<ExpressionValue, String> {
        let response_json: serde_json::Value = serde_json::from_str(response_text)
            .map_err(|_| format!("Invalid JSON response: '{}'", response_text))?;

        let value_field = response_json
            .get("value")
            .ok_or_else(|| "Missing 'value' field in response".to_string())?;

        match return_type {
            Type::String | Type::Boolean | Type::List(_) => {
                Self::parse_json_value(value_field.clone(), return_type)
            }
            Type::Option(_) => Self::parse_json_value(value_field.clone(), return_type),
            Type::Unit | Type::Custom(_) => unreachable!(),
        }
    }
}

#[async_trait(?Send)]
impl LanguageEngine for GeminiEngine {
    async fn untyped(&self, context: &Context) -> String {
        let chat_messages = self.build_context_messages(context);

        let generation_config = GenerationConfig::new()
            .with_temperature(0.9)
            .with_low_thinking();

        match self
            .client
            .structured_chat(chat_messages, self.model.clone(), Some(generation_config))
            .await
        {
            Ok(response) => response
                .first_content()
                .unwrap_or_else(|| DEFAULT_NO_RESPONSE_MESSAGE.to_string()),
            Err(e) => {
                format!("Error communicating with Gemini: {}", e)
            }
        }
    }

    async fn typed(
        &self,
        context: &Context,
        return_type: &Type,
    ) -> Result<ExpressionValue, String> {
        if matches!(return_type, Type::Unit) {
            return Ok(ExpressionValue::Unit);
        }

        let value_schema = Self::build_value_schema(return_type)?;
        let is_required = !matches!(return_type, Type::Option(_));
        let temperature = if matches!(return_type, Type::Boolean) {
            0.0
        } else {
            0.7
        };

        let schema = JsonSchemaBuilder::with_property(
            JsonSchemaBuilder::object(),
            "value",
            value_schema,
            is_required,
        );

        let mut chat_messages = self.build_context_messages(context);
        let prompt = format!("Generate a response of type '{}'", return_type.name());
        chat_messages.push(ChatMessage::user(prompt));

        let generation_config = GenerationConfig::new()
            .with_temperature(temperature)
            .with_response_mime_type("application/json".to_string())
            .with_response_schema(schema)
            .with_minimal_thinking();

        let response = self
            .client
            .structured_chat(chat_messages, self.model.clone(), Some(generation_config))
            .await
            .map_err(|e| format!("Error communicating with Gemini: {}", e))?;

        let response_text = response
            .first_content()
            .unwrap_or_else(|| DEFAULT_NO_RESPONSE_MESSAGE.to_string());

        Self::parse_typed_response(&response_text, return_type)
    }

    async fn select(&self, context: &Context, options: &[String]) -> Result<usize, String> {
        let mut selection_prompt =
            "SELECT: Choose one of the following options by responding with the appropriate number:\n"
                .to_string();
        for (index, option) in options.iter().enumerate() {
            selection_prompt.push_str(&format!("{}: {}\n", index, option));
        }

        let mut chat_messages = self.build_context_messages(context);
        chat_messages.push(ChatMessage::user(selection_prompt));

        let max_index = if options.is_empty() {
            0
        } else {
            options.len() - 1
        };

        let schema = JsonSchemaBuilder::integer_selection(max_index as u32);

        let generation_config = GenerationConfig::new()
            .with_temperature(0.0)
            .with_response_mime_type("application/json".to_string())
            .with_response_schema(schema)
            .with_minimal_thinking();

        match self
            .client
            .structured_chat(chat_messages, self.model.clone(), Some(generation_config))
            .await
        {
            Ok(response) => {
                let response_text = response
                    .first_content()
                    .unwrap_or_else(|| DEFAULT_NO_RESPONSE_MESSAGE.to_string());

                let selection_response: SelectionResponse = serde_json::from_str(&response_text)
                    .map_err(|_| {
                        format!(
                            "Invalid JSON response from language engine: '{}'",
                            response_text
                        )
                    })?;

                let selected_index = selection_response.selection as usize;

                if selected_index >= options.len() {
                    return Err(format!(
                        "Language engine selected invalid option index: {}",
                        selected_index
                    ));
                }

                Ok(selected_index)
            }
            Err(e) => Err(format!(
                "Error communicating with Gemini for selection: {}",
                e
            )),
        }
    }

    async fn fill_parameter(
        &self,
        context: &Context,
        param_name: &str,
        param_type: &Type,
    ) -> Result<ExpressionValue, String> {
        if matches!(param_type, Type::Unit) {
            return Ok(ExpressionValue::Unit);
        }

        let value_schema = Self::build_value_schema(param_type)?;
        let is_required = !matches!(param_type, Type::Option(_));
        let temperature = if matches!(param_type, Type::Boolean) {
            0.0
        } else {
            0.7
        };

        let schema = JsonSchemaBuilder::with_property(
            JsonSchemaBuilder::object(),
            "value",
            value_schema,
            is_required,
        );

        let mut chat_messages = self.build_context_messages(context);
        let prompt = format!(
            "Provide a value for parameter '{}' of type '{}'",
            param_name,
            param_type.name()
        );
        chat_messages.push(ChatMessage::user(prompt));

        let generation_config = GenerationConfig::new()
            .with_temperature(temperature)
            .with_response_mime_type("application/json".to_string())
            .with_response_schema(schema)
            .with_minimal_thinking();

        let response = self
            .client
            .structured_chat(chat_messages, self.model.clone(), Some(generation_config))
            .await
            .map_err(|e| format!("Error communicating with Gemini: {}", e))?;

        let response_text = response
            .first_content()
            .unwrap_or_else(|| DEFAULT_NO_RESPONSE_MESSAGE.to_string());

        Self::parse_typed_response(&response_text, param_type)
    }
}
