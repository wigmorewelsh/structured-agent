use crate::gemini::error::GeminiResult;
use crate::gemini::{ChatMessage, GeminiClient, GeminiConfig, ModelName};
use crate::runtime::Context;
use crate::types::LanguageEngine;
use async_trait::async_trait;

// Constants for better maintainability
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
            "SELECT: Choose one of the following options by responding with just the number:\n"
                .to_string();
        for (index, option) in options.iter().enumerate() {
            selection_prompt.push_str(&format!("{}: {}\n", index, option));
        }

        let mut chat_messages = self.build_context_messages(context);
        chat_messages.push(ChatMessage::user(selection_prompt));

        match self
            .client
            .structured_chat(chat_messages, self.model.clone(), None)
            .await
        {
            Ok(response) => {
                let selection_text = response
                    .first_content()
                    .unwrap_or_else(|| DEFAULT_NO_RESPONSE_MESSAGE.to_string());

                let selected_index: usize = selection_text.trim().parse().map_err(|_| {
                    format!(
                        "Language engine returned invalid selection: '{}'",
                        selection_text
                    )
                })?;

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
