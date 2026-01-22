use crate::gemini::error::GeminiResult;
use crate::gemini::types::GenerationConfig;
use crate::gemini::types::JsonSchema;
use crate::gemini::types::JsonSchemaProperty;
use crate::gemini::{ChatMessage, GeminiClient, GeminiConfig, ModelName};
use crate::runtime::Context;
use crate::runtime::ExprResult;
use crate::types::LanguageEngine;
use crate::types::Type;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

const DEFAULT_NO_EVENTS_MESSAGE: &str = "No events available.";
const DEFAULT_NO_RESPONSE_MESSAGE: &str = "No response received";

#[derive(Serialize, Deserialize)]
struct SelectionResponse {
    selection: u32,
}

#[derive(Serialize, Deserialize)]
struct StringResponse {
    value: String,
}

#[derive(Serialize, Deserialize)]
struct BooleanResponse {
    value: bool,
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

    fn build_context_messages(&self, context: &Context) -> Vec<ChatMessage> {
        let events: Vec<_> = context.iter_all_events().collect();

        if events.is_empty() {
            vec![ChatMessage::system(DEFAULT_NO_EVENTS_MESSAGE)]
        } else {
            events
                .iter()
                .map(|event| ChatMessage::system(&event.message))
                .collect()
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

    async fn typed(&self, context: &Context, return_type: &Type) -> Result<ExprResult, String> {
        if return_type.name == "()" {
            return Ok(ExprResult::Unit);
        }

        let (schema, temperature) = match return_type.name.as_str() {
            "String" => (
                JsonSchema::object().with_property("value", JsonSchemaProperty::string(), true),
                0.7,
            ),
            "Boolean" => (
                JsonSchema::object().with_property("value", JsonSchemaProperty::boolean(), true),
                0.0,
            ),
            _ => return Err(format!("Unsupported return type: {}", return_type.name)),
        };

        let mut chat_messages = self.build_context_messages(context);
        let prompt = format!("Generate a response of type '{}'", return_type.name);
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

        match return_type.name.as_str() {
            "String" => {
                let string_response: StringResponse = serde_json::from_str(&response_text)
                    .map_err(|_| format!("Invalid JSON response: '{}'", response_text))?;
                Ok(ExprResult::String(string_response.value))
            }
            "Boolean" => {
                let boolean_response: BooleanResponse = serde_json::from_str(&response_text)
                    .map_err(|_| format!("Invalid JSON response: '{}'", response_text))?;
                Ok(ExprResult::Boolean(boolean_response.value))
            }
            _ => unreachable!(),
        }
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

        let schema = JsonSchema::integer_selection(max_index as u32);

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
    ) -> Result<ExprResult, String> {
        if param_type.name == "()" {
            return Ok(ExprResult::Unit);
        }

        let (schema, temperature) = match param_type.name.as_str() {
            "String" => (
                JsonSchema::object().with_property("value", JsonSchemaProperty::string(), true),
                0.7,
            ),
            "Boolean" => (
                JsonSchema::object().with_property("value", JsonSchemaProperty::boolean(), true),
                0.0,
            ),
            _ => return Err(format!("Unsupported parameter type: {}", param_type.name)),
        };

        let mut chat_messages = self.build_context_messages(context);
        let prompt = format!(
            "Provide a value for parameter '{}' of type '{}'",
            param_name, param_type.name
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

        match param_type.name.as_str() {
            "String" => {
                let string_response: StringResponse = serde_json::from_str(&response_text)
                    .map_err(|_| format!("Invalid JSON response: '{}'", response_text))?;
                Ok(ExprResult::String(string_response.value))
            }
            "Boolean" => {
                let boolean_response: BooleanResponse = serde_json::from_str(&response_text)
                    .map_err(|_| format!("Invalid JSON response: '{}'", response_text))?;
                Ok(ExprResult::Boolean(boolean_response.value))
            }
            _ => unreachable!(),
        }
    }
}
