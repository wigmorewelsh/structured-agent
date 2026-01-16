use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelName {
    Gemini25Pro,
    Gemini25Flash,
    Gemini25FlashLite,
    Gemini3FlashPreview,
    Gemini3ProPreview,
    Custom(String),
}

impl ModelName {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Gemini25Pro => "gemini-2.5-pro",
            Self::Gemini25Flash => "gemini-2.5-flash",
            Self::Gemini25FlashLite => "gemini-2.5-flash-lite",
            Self::Gemini3FlashPreview => "gemini-3-flash-preview",
            Self::Gemini3ProPreview => "gemini-3-pro-preview",
            Self::Custom(name) => name,
        }
    }

    pub fn full_name(&self, project_id: &str, location: &str) -> String {
        format!(
            "projects/{}/locations/{}/publishers/google/models/{}",
            project_id,
            location,
            self.as_str()
        )
    }
}

impl Default for ModelName {
    fn default() -> Self {
        Self::Gemini25Flash
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "system")]
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            metadata: None,
        }
    }

    pub fn model(content: impl Into<String>) -> Self {
        Self {
            role: Role::Model,
            content: content.into(),
            metadata: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<u32>,
}

impl GenerationConfig {
    pub fn new() -> Self {
        Self {
            temperature: None,
            top_k: None,
            top_p: None,
            max_output_tokens: None,
            stop_sequences: None,
            candidate_count: None,
        }
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature.clamp(0.0, 2.0));
        self
    }

    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p.clamp(0.0, 1.0));
        self
    }

    pub fn with_max_output_tokens(mut self, max_tokens: u32) -> Self {
        self.max_output_tokens = Some(max_tokens);
        self
    }

    pub fn with_stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: ModelName,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<String>,
}

impl ChatRequest {
    pub fn new(messages: Vec<ChatMessage>, model: ModelName) -> Self {
        Self {
            messages,
            model,
            generation_config: None,
            system_instruction: None,
        }
    }

    pub fn with_generation_config(mut self, config: GenerationConfig) -> Self {
        self.generation_config = Some(config);
        self
    }

    pub fn with_system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.system_instruction = Some(instruction.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
    pub blocked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetadata {
    pub prompt_token_count: Option<u32>,
    pub candidates_token_count: Option<u32>,
    pub total_token_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Vec<Candidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_feedback: Option<Value>,
}

impl GeminiResponse {
    pub fn first_content(&self) -> Option<&str> {
        self.candidates
            .first()
            .map(|candidate| candidate.content.as_str())
    }

    pub fn is_blocked(&self) -> bool {
        self.candidates.iter().any(|candidate| {
            candidate.safety_ratings.as_ref().is_some_and(|ratings| {
                ratings.iter().any(|rating| rating.blocked.unwrap_or(false))
            })
        })
    }

    pub fn token_count(&self) -> Option<u32> {
        self.usage_metadata
            .as_ref()
            .and_then(|metadata| metadata.total_token_count)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingResponse {
    pub content: String,
    pub is_complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
}
