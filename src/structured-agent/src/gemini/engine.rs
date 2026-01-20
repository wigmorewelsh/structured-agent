use crate::gemini::error::GeminiResult;
use crate::gemini::types::GenerationConfig;
use crate::gemini::types::JsonSchema;
use crate::gemini::{ChatMessage, GeminiClient, GeminiConfig, ModelName};
use crate::runtime::Context;
use crate::types::LanguageEngine;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// Constants for better maintainability
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

    fn build_context_messages(&self, context: &Context) -> Vec<ChatMessage> {
        if context.events.is_empty() {
            vec![ChatMessage::system(DEFAULT_NO_EVENTS_MESSAGE)]
        } else {
            context
                .events
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

        match self
            .client
            .structured_chat(chat_messages, self.model.clone(), None)
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
            .with_response_schema(schema);

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
}
