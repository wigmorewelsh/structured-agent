use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

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
pub struct ThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none", rename = "thinkingLevel")]
    pub thinking_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "thinkingBudget")]
    pub thinking_budget: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "includeThoughts")]
    pub include_thoughts: Option<bool>,
}

impl ThinkingConfig {
    pub fn minimal() -> Self {
        Self {
            thinking_level: None,
            thinking_budget: Some(0),
            include_thoughts: None,
        }
    }

    pub fn low() -> Self {
        Self {
            thinking_level: None,
            thinking_budget: Some(512),
            include_thoughts: None,
        }
    }

    pub fn medium() -> Self {
        Self {
            thinking_level: Some("medium".to_string()),
            thinking_budget: None,
            include_thoughts: None,
        }
    }

    pub fn high() -> Self {
        Self {
            thinking_level: Some("high".to_string()),
            thinking_budget: None,
            include_thoughts: None,
        }
    }

    pub fn disabled() -> Self {
        Self {
            thinking_level: None,
            thinking_budget: Some(0),
            include_thoughts: None,
        }
    }

    pub fn with_budget(budget: i32) -> Self {
        Self {
            thinking_level: None,
            thinking_budget: Some(budget),
            include_thoughts: None,
        }
    }

    pub fn with_include_thoughts(mut self, include: bool) -> Self {
        self.include_thoughts = Some(include);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topK")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topP")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxOutputTokens")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "stopSequences")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "candidateCount")]
    pub candidate_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "responseMimeType")]
    pub response_mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "responseSchema")]
    pub response_schema: Option<JsonSchema>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "thinkingConfig")]
    pub thinking_config: Option<ThinkingConfig>,
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
            response_mime_type: None,
            response_schema: None,
            thinking_config: None,
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

    pub fn with_response_mime_type(mut self, mime_type: String) -> Self {
        self.response_mime_type = Some(mime_type);
        self
    }

    pub fn with_response_schema(mut self, schema: JsonSchema) -> Self {
        self.response_schema = Some(schema);
        self
    }

    pub fn with_thinking_config(mut self, thinking_config: ThinkingConfig) -> Self {
        self.thinking_config = Some(thinking_config);
        self
    }

    pub fn with_minimal_thinking(mut self) -> Self {
        self.thinking_config = Some(ThinkingConfig::minimal());
        self
    }

    pub fn with_low_thinking(mut self) -> Self {
        self.thinking_config = Some(ThinkingConfig::low());
        self
    }

    pub fn without_thinking(mut self) -> Self {
        self.thinking_config = Some(ThinkingConfig::disabled());
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
    pub blocked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    pub prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    pub candidates_token_count: Option<u32>,
    #[serde(rename = "totalTokenCount")]
    pub total_token_count: Option<u32>,
    #[serde(rename = "thoughtsTokenCount")]
    pub thoughts_token_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseContent {
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    pub content: ResponseContent,
    #[serde(skip_serializing_if = "Option::is_none", rename = "finishReason")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "safetyRatings")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "citationMetadata")]
    pub citation_metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeminiResponse {
    pub candidates: Vec<Candidate>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "usageMetadata")]
    pub usage_metadata: Option<UsageMetadata>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "promptFeedback")]
    pub prompt_feedback: Option<Value>,
}

impl GeminiResponse {
    pub fn first_content(&self) -> Option<String> {
        self.candidates.first().map(|candidate| {
            candidate
                .content
                .parts
                .iter()
                .map(|part| part.text.as_str())
                .collect::<Vec<_>>()
                .join("")
        })
    }

    pub fn is_blocked(&self) -> bool {
        self.candidates.iter().any(|candidate| {
            candidate
                .safety_ratings
                .as_ref()
                .is_some_and(|ratings| ratings.iter().any(|rating| rating.blocked.unwrap_or(false)))
        })
    }

    pub fn token_count(&self) -> Option<u32> {
        self.usage_metadata
            .as_ref()
            .and_then(|metadata| metadata.total_token_count)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamingResponse {
    pub content: String,
    pub is_complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Part {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

pub type JsonSchema = SchemaObject;

pub struct JsonSchemaBuilder;

impl JsonSchemaBuilder {
    pub fn integer_selection(max_value: u32) -> SchemaObject {
        let mut schema = SchemaObject::default();
        schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::Object)));

        let mut properties = BTreeMap::new();
        let mut selection_schema = SchemaObject::default();
        selection_schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::Integer)));
        let number_validation = selection_schema.number();
        number_validation.minimum = Some(0.0);
        number_validation.maximum = Some(max_value as f64);
        properties.insert("selection".to_string(), Schema::Object(selection_schema));

        let object_validation = schema.object();
        object_validation.properties = properties;
        object_validation.required.insert("selection".to_string());
        schema
    }

    pub fn object() -> SchemaObject {
        let mut schema = SchemaObject::default();
        schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::Object)));
        schema
    }

    pub fn with_property(
        mut schema: SchemaObject,
        name: &str,
        property_schema: SchemaObject,
        required: bool,
    ) -> SchemaObject {
        let object_validation = schema.object();
        object_validation
            .properties
            .insert(name.to_string(), Schema::Object(property_schema));
        if required {
            object_validation.required.insert(name.to_string());
        }
        schema
    }

    pub fn string() -> SchemaObject {
        let mut schema = SchemaObject::default();
        schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::String)));
        schema
    }

    pub fn boolean() -> SchemaObject {
        let mut schema = SchemaObject::default();
        schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::Boolean)));
        schema
    }

    pub fn array(items: SchemaObject) -> SchemaObject {
        let mut schema = SchemaObject::default();
        schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::Array)));
        let array_validation = schema.array();
        array_validation.items = Some(SingleOrVec::Single(Box::new(Schema::Object(items))));
        schema
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInstruction {
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiApiRequest {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "generationConfig")]
    pub generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "systemInstruction")]
    pub system_instruction: Option<SystemInstruction>,
}

impl From<&ChatRequest> for GeminiApiRequest {
    fn from(request: &ChatRequest) -> Self {
        let contents = request
            .messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Model => "model",
                    Role::System => "user",
                };

                Content {
                    role: role.to_string(),
                    parts: vec![Part {
                        text: msg.content.clone(),
                    }],
                }
            })
            .collect();

        let system_instruction =
            request
                .system_instruction
                .as_ref()
                .map(|instruction| SystemInstruction {
                    parts: vec![Part {
                        text: instruction.clone(),
                    }],
                });

        Self {
            contents,
            generation_config: request.generation_config.clone(),
            system_instruction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_generation_config_serialization() {
        let config = GenerationConfig {
            temperature: Some(0.7),
            top_k: Some(40),
            top_p: Some(0.95),
            max_output_tokens: Some(1024),
            stop_sequences: Some(vec!["STOP".to_string(), "END".to_string()]),
            candidate_count: Some(1),
            response_mime_type: None,
            response_schema: None,
            thinking_config: None,
        };

        let serialized = serde_json::to_value(&config).unwrap();

        assert_eq!(serialized["topK"], json!(40));
        assert_eq!(serialized["maxOutputTokens"], json!(1024));
        assert_eq!(serialized["stopSequences"], json!(["STOP", "END"]));
        assert_eq!(serialized["candidateCount"], json!(1));

        let temp = serialized["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.001);

        let top_p = serialized["topP"].as_f64().unwrap();
        assert!((top_p - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_generation_config_partial_serialization() {
        let config = GenerationConfig {
            temperature: Some(0.5),
            top_k: None,
            top_p: Some(0.8),
            max_output_tokens: Some(512),
            stop_sequences: None,
            candidate_count: Some(1),
            response_mime_type: None,
            response_schema: None,
            thinking_config: None,
        };

        let serialized = serde_json::to_value(&config).unwrap();

        assert!(serialized.get("topK").is_none());
        assert!(serialized.get("stopSequences").is_none());

        assert!(serialized.get("maxOutputTokens").is_some());
        assert!(serialized.get("candidateCount").is_some());

        let temp = serialized["temperature"].as_f64().unwrap();
        assert!((temp - 0.5).abs() < 0.001);

        let top_p = serialized["topP"].as_f64().unwrap();
        assert!((top_p - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_gemini_api_request_serialization() {
        let messages = vec![
            ChatMessage::user("Hello, how are you?"),
            ChatMessage::model("I'm doing well, thank you!"),
        ];

        let generation_config = GenerationConfig {
            temperature: Some(0.7),
            top_k: Some(40),
            top_p: None,
            max_output_tokens: Some(1024),
            stop_sequences: None,
            candidate_count: None,
            response_mime_type: None,
            response_schema: None,
            thinking_config: None,
        };

        let request = ChatRequest {
            messages,
            model: ModelName::Gemini25Flash,
            generation_config: Some(generation_config),
            system_instruction: Some("You are a helpful assistant.".to_string()),
        };

        let api_request = GeminiApiRequest::from(&request);
        let serialized = serde_json::to_value(&api_request).unwrap();

        let contents = serialized["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 2);

        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "Hello, how are you?");

        assert_eq!(contents[1]["role"], "model");
        assert_eq!(
            contents[1]["parts"][0]["text"],
            "I'm doing well, thank you!"
        );

        assert!(serialized.get("generationConfig").is_some());
        let gen_config = &serialized["generationConfig"];
        assert_eq!(gen_config["topK"], 40);
        assert_eq!(gen_config["maxOutputTokens"], 1024);
        assert!(gen_config.get("topP").is_none());

        assert!(serialized.get("systemInstruction").is_some());
        let sys_instruction = &serialized["systemInstruction"];
        assert_eq!(
            sys_instruction["parts"][0]["text"],
            "You are a helpful assistant."
        );
    }

    #[test]
    fn test_gemini_api_request_minimal() {
        let messages = vec![ChatMessage::user("Simple test")];

        let request = ChatRequest {
            messages,
            model: ModelName::Gemini25Flash,
            generation_config: None,
            system_instruction: None,
        };

        let api_request = GeminiApiRequest::from(&request);
        let serialized = serde_json::to_value(&api_request).unwrap();

        assert!(serialized.get("contents").is_some());
        assert!(serialized.get("generationConfig").is_none());
        assert!(serialized.get("systemInstruction").is_none());

        let contents = serialized["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "Simple test");
    }

    #[test]
    fn test_gemini_response_deserialization() {
        let response_json = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello! How can I help you today?"}]
                },
                "finishReason": "STOP",
                "safetyRatings": [{
                    "category": "HARM_CATEGORY_HARASSMENT",
                    "probability": "NEGLIGIBLE",
                    "blocked": false
                }],
                "citationMetadata": null
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 15,
                "totalTokenCount": 25
            },
            "promptFeedback": null
        });

        let response: GeminiResponse = serde_json::from_value(response_json).unwrap();

        assert_eq!(response.candidates.len(), 1);
        assert_eq!(response.candidates[0].content.parts.len(), 1);
        assert_eq!(
            response.candidates[0].content.parts[0].text,
            "Hello! How can I help you today?"
        );
        assert_eq!(
            response.candidates[0].finish_reason,
            Some("STOP".to_string())
        );

        let safety_ratings = response.candidates[0].safety_ratings.as_ref().unwrap();
        assert_eq!(safety_ratings.len(), 1);
        assert_eq!(safety_ratings[0].category, "HARM_CATEGORY_HARASSMENT");
        assert_eq!(safety_ratings[0].probability, "NEGLIGIBLE");
        assert_eq!(safety_ratings[0].blocked, Some(false));

        let usage = response.usage_metadata.as_ref().unwrap();
        assert_eq!(usage.prompt_token_count, Some(10));
        assert_eq!(usage.candidates_token_count, Some(15));
        assert_eq!(usage.total_token_count, Some(25));
    }

    #[test]
    fn test_gemini_response_minimal() {
        let response_json = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Short response"}]
                }
            }]
        });

        let response: GeminiResponse = serde_json::from_value(response_json).unwrap();

        assert_eq!(response.candidates.len(), 1);
        assert_eq!(
            response.candidates[0].content.parts[0].text,
            "Short response"
        );
        assert_eq!(response.candidates[0].finish_reason, None);
        assert_eq!(response.candidates[0].safety_ratings, None);
        assert_eq!(response.usage_metadata, None);
    }

    #[test]
    fn test_first_content_method() {
        let response = GeminiResponse {
            candidates: vec![Candidate {
                content: ResponseContent {
                    parts: vec![
                        Part {
                            text: "Hello ".to_string(),
                        },
                        Part {
                            text: "world!".to_string(),
                        },
                    ],
                },
                finish_reason: None,
                safety_ratings: None,
                citation_metadata: None,
            }],
            usage_metadata: None,
            prompt_feedback: None,
        };

        assert_eq!(response.first_content(), Some("Hello world!".to_string()));
    }

    #[test]
    fn test_first_content_empty() {
        let response = GeminiResponse {
            candidates: vec![],
            usage_metadata: None,
            prompt_feedback: None,
        };

        assert_eq!(response.first_content(), None);
    }
}
