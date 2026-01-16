use crate::gemini::{ChatMessage, GeminiClient, GeminiConfig, ModelName};
use crate::runtime::Context;
use crate::types::LanguageEngine;
use async_trait::async_trait;

pub struct GeminiEngine {
    client: GeminiClient,
    model: ModelName,
}

impl GeminiEngine {
    pub async fn new(config: GeminiConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let client = GeminiClient::new(config).await?;

        Ok(Self {
            client,
            model: ModelName::default(),
        })
    }

    pub async fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
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
            vec![ChatMessage::system("No events available.")]
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
                .unwrap_or("No response received")
                .to_string(),
            Err(e) => {
                format!("Error communicating with Gemini: {}", e)
            }
        }
    }
}
