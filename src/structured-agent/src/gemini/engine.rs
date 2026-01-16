use crate::gemini::{ChatMessage, GeminiClient, GeminiConfig, ModelName};
use crate::types::{Context, LanguageEngine};
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

    fn build_context_message(&self, context: &Context) -> String {
        let mut message = String::new();

        if !context.events.is_empty() {
            message.push_str("Recent events:\n");
            for event in &context.events {
                message.push_str(&format!("- {}\n", event.message));
            }
            message.push('\n');
        }

        if !context.variables.is_empty() {
            message.push_str("Available variables:\n");
            for (name, value) in &context.variables {
                match value {
                    crate::types::ExprResult::String(s) => {
                        message.push_str(&format!("- {}: \"{}\"\n", name, s));
                    }
                    crate::types::ExprResult::Unit => {
                        message.push_str(&format!("- {}: ()\n", name));
                    }
                }
            }
            message.push('\n');
        }

        let functions = context.registry.list_functions();
        if !functions.is_empty() {
            message.push_str("Available functions:\n");
            for func_name in functions {
                message.push_str(&format!("- {}\n", func_name));
            }
        }

        if message.is_empty() {
            "No context information available.".to_string()
        } else {
            message
        }
    }
}

impl LanguageEngine for GeminiEngine {
    fn untyped(&self, context: &Context) -> String {
        let context_message = self.build_context_message(context);
        let chat_message = ChatMessage::user(context_message);

        match self.runtime.block_on(self.client.structured_chat(
            vec![chat_message],
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
