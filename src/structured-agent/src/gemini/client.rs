use crate::gemini::{
    config::{AuthMethod, GeminiConfig},
    error::{GeminiError, GeminiResult},
    types::{
        Candidate, ChatMessage, ChatRequest, GeminiResponse, GenerationConfig, ModelName, Role,
        SafetyRating, UsageMetadata,
    },
};
use serde_json::{Value, json};
use std::process::Command;
use std::time::Duration;
use tokio::time::timeout;

pub struct GeminiClient {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    config: GeminiConfig,
}

impl GeminiClient {
    pub async fn new(config: GeminiConfig) -> GeminiResult<Self> {
        config.validate().map_err(GeminiError::Configuration)?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| GeminiError::Network(e.to_string()))?;

        let (api_key, base_url) = match &config.auth_method {
            AuthMethod::ApiKey(key) => {
                let base_url = config
                    .api_endpoint
                    .clone()
                    .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string());
                (Some(key.clone()), base_url)
            }
            AuthMethod::ApplicationDefaultCredentials => {
                let base_url = config.api_endpoint.clone().unwrap_or_else(|| {
                    format!("https://{}-aiplatform.googleapis.com", config.location)
                });

                (None, base_url)
            }
        };

        Ok(Self {
            client,
            api_key,
            base_url,
            config,
        })
    }

    pub async fn from_env() -> GeminiResult<Self> {
        let config =
            GeminiConfig::from_env().map_err(|e| GeminiError::Configuration(e.to_string()))?;
        Self::new(config).await
    }

    pub async fn chat(&self, request: ChatRequest) -> GeminiResult<GeminiResponse> {
        self.chat_with_timeout(request, Duration::from_secs(30))
            .await
    }

    pub async fn chat_with_timeout(
        &self,
        request: ChatRequest,
        timeout_duration: Duration,
    ) -> GeminiResult<GeminiResponse> {
        timeout(timeout_duration, self.chat_internal(request))
            .await
            .map_err(|_| GeminiError::Timeout)?
    }

    async fn chat_internal(&self, request: ChatRequest) -> GeminiResult<GeminiResponse> {
        let (_url, request_builder) = match &self.config.auth_method {
            AuthMethod::ApiKey(_) => {
                let url = format!(
                    "{}/v1beta/models/{}:generateContent",
                    self.base_url,
                    request.model.as_str()
                );
                let api_key = self
                    .api_key
                    .as_ref()
                    .ok_or_else(|| GeminiError::Configuration("API key not set".to_string()))?;
                let builder = self.client.post(&url).query(&[("key", api_key)]);
                (url, builder)
            }
            AuthMethod::ApplicationDefaultCredentials => {
                let url = format!(
                    "{}/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
                    self.base_url,
                    self.config.project_id,
                    self.config.location,
                    request.model.as_str()
                );

                let token = self.get_gcloud_token().await?;

                let builder = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", token));
                (url, builder)
            }
        };

        let payload = self.build_request_payload(&request)?;

        let response = request_builder
            .json(&payload)
            .send()
            .await
            .map_err(|e| GeminiError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = match response.text().await {
                Ok(text) => text,
                Err(e) => format!("Failed to read error response: {}", e),
            };

            return Err(self.map_http_error(status.as_u16(), error_text));
        }

        let response_body: Value = response
            .json()
            .await
            .map_err(|e| GeminiError::Serialization(e.to_string()))?;

        self.parse_response(response_body)
    }

    pub async fn simple_chat(&self, message: impl Into<String>) -> GeminiResult<String> {
        let chat_message = ChatMessage::user(message);
        let request = ChatRequest::new(vec![chat_message], ModelName::default());

        let response = self.chat(request).await?;
        response
            .first_content()
            .ok_or_else(|| GeminiError::ApiError {
                code: 0,
                message: "No response content received".to_string(),
            })
            .map(|s| s.to_string())
    }

    pub async fn structured_chat(
        &self,
        messages: Vec<ChatMessage>,
        model: ModelName,
        config: Option<GenerationConfig>,
    ) -> GeminiResult<GeminiResponse> {
        let mut request = ChatRequest::new(messages, model);

        if let Some(gen_config) = config {
            request = request.with_generation_config(gen_config);
        }

        self.chat(request).await
    }

    fn build_request_payload(&self, request: &ChatRequest) -> GeminiResult<Value> {
        let contents: Vec<Value> = request
            .messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Model => "model",
                    Role::System => "user",
                };

                json!({
                    "role": role,
                    "parts": [{"text": msg.content}]
                })
            })
            .collect();

        let mut payload = json!({
            "contents": contents
        });

        if let Some(gen_config) = &request.generation_config {
            let mut generation_config = json!({});

            if let Some(temperature) = gen_config.temperature {
                generation_config["temperature"] = json!(temperature);
            }
            if let Some(top_k) = gen_config.top_k {
                generation_config["topK"] = json!(top_k);
            }
            if let Some(top_p) = gen_config.top_p {
                generation_config["topP"] = json!(top_p);
            }
            if let Some(max_tokens) = gen_config.max_output_tokens {
                generation_config["maxOutputTokens"] = json!(max_tokens);
            }
            if let Some(stop_sequences) = &gen_config.stop_sequences {
                generation_config["stopSequences"] = json!(stop_sequences);
            }

            payload["generationConfig"] = generation_config;
        }

        if let Some(system_instruction) = &request.system_instruction {
            payload["systemInstruction"] = json!({
                "parts": [{"text": system_instruction}]
            });
        }

        Ok(payload)
    }

    fn parse_response(&self, response: Value) -> GeminiResult<GeminiResponse> {
        let candidates_array = response
            .get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| GeminiError::ApiError {
                code: 0,
                message: "Invalid response format: missing candidates".to_string(),
            })?;

        let candidates: Vec<Candidate> = candidates_array
            .iter()
            .map(|candidate| {
                let content = candidate
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .unwrap_or_default();

                let finish_reason = candidate
                    .get("finishReason")
                    .and_then(|fr| fr.as_str())
                    .map(|s| s.to_string());

                let safety_ratings = candidate
                    .get("safetyRatings")
                    .and_then(|sr| sr.as_array())
                    .map(|ratings| {
                        ratings
                            .iter()
                            .filter_map(|rating| {
                                Some(SafetyRating {
                                    category: rating.get("category")?.as_str()?.to_string(),
                                    probability: rating.get("probability")?.as_str()?.to_string(),
                                    blocked: rating.get("blocked").and_then(|b| b.as_bool()),
                                })
                            })
                            .collect()
                    });

                Candidate {
                    content,
                    finish_reason,
                    safety_ratings,
                    citation_metadata: candidate.get("citationMetadata").cloned(),
                }
            })
            .collect();

        let usage_metadata = response.get("usageMetadata").map(|metadata| UsageMetadata {
            prompt_token_count: metadata
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            candidates_token_count: metadata
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            total_token_count: metadata
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        });

        Ok(GeminiResponse {
            candidates,
            usage_metadata,
            prompt_feedback: response.get("promptFeedback").cloned(),
        })
    }

    fn map_http_error(&self, status_code: u16, error_message: String) -> GeminiError {
        match status_code {
            400 => GeminiError::InvalidInput(error_message),
            401 => GeminiError::Authentication("Invalid API key".to_string()),
            403 => GeminiError::Authentication("Permission denied or quota exceeded".to_string()),
            404 => GeminiError::ModelNotFound(error_message),
            429 => GeminiError::RateLimited,
            500..=599 => GeminiError::ApiError {
                code: status_code as u32,
                message: error_message,
            },
            _ => GeminiError::Unknown(format!("HTTP {}: {}", status_code, error_message)),
        }
    }

    pub fn config(&self) -> &GeminiConfig {
        &self.config
    }

    async fn get_gcloud_token(&self) -> GeminiResult<String> {
        let output = Command::new("gcloud")
            .args(&["auth", "print-access-token"])
            .output()
            .map_err(|e| {
                GeminiError::Authentication(format!(
                    "Failed to run gcloud command: {}. Make sure gcloud is installed and you've run 'gcloud auth application-default login'",
                    e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GeminiError::Authentication(format!(
                "gcloud auth failed: {}",
                stderr
            )));
        }

        let token_str = String::from_utf8_lossy(&output.stdout);
        let token = token_str.trim();

        if token.is_empty() {
            return Err(GeminiError::Authentication(
                "Empty token received from gcloud".to_string(),
            ));
        }

        Ok(token.to_string())
    }

    pub fn is_using_adc(&self) -> bool {
        matches!(
            self.config.auth_method,
            AuthMethod::ApplicationDefaultCredentials
        )
    }

    pub fn is_using_api_key(&self) -> bool {
        matches!(self.config.auth_method, AuthMethod::ApiKey(_))
    }
}
