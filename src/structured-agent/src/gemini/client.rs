use crate::gemini::{
    config::{AuthMethod, GeminiConfig},
    error::{GeminiError, GeminiResult},
    types::{
        ChatMessage, ChatRequest, GeminiApiRequest, GeminiResponse, GenerationConfig, ModelName,
    },
};
use serde_json::Value;

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::timeout;
use url::Url;

// Constants for better maintainability
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_API_BASE: &str = "https://generativelanguage.googleapis.com";
const DEFAULT_VERTEX_BASE: &str = "https://{location}-aiplatform.googleapis.com";
const GCLOUD_AUTH_COMMAND: &[&str] = &["auth", "print-access-token"];

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    expires_at: SystemTime,
}

impl CachedToken {
    fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }
}

pub struct GeminiClient {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    config: GeminiConfig,
    cached_token: Arc<RwLock<Option<CachedToken>>>,
}

impl GeminiClient {
    pub async fn new(config: GeminiConfig) -> GeminiResult<Self> {
        config.validate().map_err(GeminiError::Configuration)?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| GeminiError::Network(e.to_string()))?;

        let (api_key, base_url) = match &config.auth_method {
            AuthMethod::ApiKey(key) => {
                let base_url = config
                    .api_endpoint
                    .clone()
                    .unwrap_or_else(|| DEFAULT_API_BASE.to_string());
                (Some(key.clone()), base_url)
            }
            AuthMethod::ApplicationDefaultCredentials => {
                let base_url = config
                    .api_endpoint
                    .clone()
                    .unwrap_or_else(|| DEFAULT_VERTEX_BASE.replace("{location}", &config.location));

                (None, base_url)
            }
        };

        Ok(Self {
            client,
            api_key,
            base_url,
            config,
            cached_token: Arc::new(RwLock::new(None)),
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
                let url = self.build_api_url(&request.model)?;
                let api_key = self
                    .api_key
                    .as_ref()
                    .ok_or_else(|| GeminiError::Configuration("API key not set".to_string()))?;
                let builder = self.client.post(&url).query(&[("key", api_key)]);
                (url, builder)
            }
            AuthMethod::ApplicationDefaultCredentials => {
                let url = self.build_vertex_url(&request.model)?;
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
        let response = self
            .structured_chat(vec![chat_message], ModelName::default(), None)
            .await?;

        response
            .first_content()
            .ok_or_else(|| GeminiError::ApiError {
                code: 0,
                message: "No response content received".to_string(),
            })
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
        let api_request = GeminiApiRequest::from(request);
        serde_json::to_value(&api_request).map_err(Into::into)
    }

    fn parse_response(&self, response: Value) -> GeminiResult<GeminiResponse> {
        serde_json::from_value(response).map_err(Into::into)
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

    fn build_api_url(&self, model: &ModelName) -> GeminiResult<String> {
        let mut url = Url::parse(&self.base_url)
            .map_err(|e| GeminiError::Configuration(format!("Invalid base URL: {}", e)))?;

        url.path_segments_mut()
            .map_err(|_| GeminiError::Configuration("Cannot be base URL".to_string()))?
            .extend(&[
                "v1beta",
                "models",
                &format!("{}:generateContent", model.as_str()),
            ]);

        Ok(url.to_string())
    }

    fn build_vertex_url(&self, model: &ModelName) -> GeminiResult<String> {
        let mut url = Url::parse(&self.base_url)
            .map_err(|e| GeminiError::Configuration(format!("Invalid base URL: {}", e)))?;

        url.path_segments_mut()
            .map_err(|_| GeminiError::Configuration("Cannot be base URL".to_string()))?
            .extend(&[
                "v1",
                "projects",
                &self.config.project_id,
                "locations",
                &self.config.location,
                "publishers",
                "google",
                "models",
                &format!("{}:generateContent", model.as_str()),
            ]);

        Ok(url.to_string())
    }

    async fn get_gcloud_token(&self) -> GeminiResult<String> {
        // Check if we have a valid cached token
        {
            let cached_token = self.cached_token.read().await;
            if let Some(ref token_data) = *cached_token {
                if !token_data.is_expired() {
                    return Ok(token_data.token.clone());
                }
            }
        }

        // Need to fetch a new token
        let output = tokio::process::Command::new("gcloud")
            .args(GCLOUD_AUTH_COMMAND)
            .output()
            .await
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

        // Cache the token (expires in 55 minutes to be safe)
        let cached_token_data = CachedToken {
            token: token.to_string(),
            expires_at: SystemTime::now() + Duration::from_secs(55 * 60),
        };

        {
            let mut cached_token = self.cached_token.write().await;
            *cached_token = Some(cached_token_data.clone());
        }

        Ok(cached_token_data.token)
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
