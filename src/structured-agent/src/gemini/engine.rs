use crate::gemini::{ChatMessage, GeminiClient, GeminiConfig, ModelName};
use crate::runtime::Context;
use crate::types::LanguageEngine;
use tokio::runtime::Runtime;

pub struct GeminiEngine {
    client: GeminiClient,
    model: ModelName,
    runtime: Runtime,
}

impl GeminiEngine {
    pub fn new(config: GeminiConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let runtime = Runtime::new()?;
        let client = runtime.block_on(GeminiClient::new(config))?;

        Ok(Self {
            client,
            model: ModelName::default(),
            runtime,
        })
    }

    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let runtime = Runtime::new()?;
        let client = runtime.block_on(GeminiClient::from_env())?;

        Ok(Self {
            client,
            model: ModelName::default(),
            runtime,
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

impl LanguageEngine for GeminiEngine {
    fn untyped(&self, context: &Context) -> String {
        let chat_messages = self.build_context_messages(context);

        match self.runtime.block_on(self.client.structured_chat(
            chat_messages,
            self.model.clone(),
            None,
        )) {
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
