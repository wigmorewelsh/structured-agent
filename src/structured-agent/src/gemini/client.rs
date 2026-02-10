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

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 90;
const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY_MS: u64 = 1000;
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
    request_timeout: Duration,
    max_retries: u32,
}

impl GeminiClient {
    pub async fn new(config: GeminiConfig) -> GeminiResult<Self> {
        config.validate().map_err(GeminiError::Configuration)?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .tcp_keepalive(Duration::from_secs(60))
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
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            max_retries: MAX_RETRIES,
        })
    }

    pub async fn from_env() -> GeminiResult<Self> {
        let config =
            GeminiConfig::from_env().map_err(|e| GeminiError::Configuration(e.to_string()))?;
        Self::new(config).await
    }

    pub async fn chat(&self, request: ChatRequest) -> GeminiResult<GeminiResponse> {
        self.chat_with_timeout(request, self.request_timeout).await
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub async fn chat_with_timeout(
        &self,
        request: ChatRequest,
        timeout_duration: Duration,
    ) -> GeminiResult<GeminiResponse> {
        let mut last_error = None;
        let mut retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);

        for attempt in 0..=self.max_retries {
            match timeout(timeout_duration, self.chat_internal(request.clone())).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(e)) => {
                    let (should_retry, custom_delay) = match &e {
                        GeminiError::RateLimited | GeminiError::RateLimitedWithRetry(_) => {
                            (true, self.extract_retry_delay(&e))
                        }
                        GeminiError::Timeout | GeminiError::Network(_) => (true, None),
                        GeminiError::ApiError {
                            code: 500..=599, ..
                        } => (true, None),
                        _ => (false, None),
                    };

                    if should_retry && attempt < self.max_retries {
                        last_error = Some(e);
                        let delay = custom_delay.unwrap_or(retry_delay);
                        tokio::time::sleep(delay).await;
                        retry_delay *= 2;
                        continue;
                    }
                    return Err(e);
                }
                Err(_) => {
                    if attempt < self.max_retries {
                        last_error = Some(GeminiError::Timeout);
                        tokio::time::sleep(retry_delay).await;
                        retry_delay *= 2;
                        continue;
                    }
                    return Err(GeminiError::Timeout);
                }
            }
        }

        Err(last_error.unwrap_or(GeminiError::Timeout))
    }

    fn extract_retry_delay(&self, error: &GeminiError) -> Option<Duration> {
        match error {
            GeminiError::RateLimitedWithRetry(duration) => Some(*duration),
            _ => None,
        }
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

        let response = request_builder.json(&payload).send().await.map_err(|e| {
            if e.is_timeout() {
                GeminiError::Timeout
            } else if e.is_connect() {
                GeminiError::Network(format!("Connection failed: {}", e))
            } else {
                GeminiError::Network(e.to_string())
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let headers = response.headers().clone();
            let error_text = match response.text().await {
                Ok(text) => text,
                Err(e) => format!("Failed to read error response: {}", e),
            };

            return Err(self.map_http_error(status.as_u16(), error_text, headers));
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

    fn map_http_error(
        &self,
        status_code: u16,
        error_message: String,
        headers: reqwest::header::HeaderMap,
    ) -> GeminiError {
        match status_code {
            400 => GeminiError::InvalidInput(error_message),
            401 => GeminiError::Authentication("Invalid API key".to_string()),
            403 => GeminiError::Authentication("Permission denied or quota exceeded".to_string()),
            404 => GeminiError::ModelNotFound(error_message),
            429 => {
                let retry_after = headers
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());

                let rate_limit_reset = headers
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());

                let mut error = GeminiError::RateLimited;
                if let Some(seconds) = retry_after.or(rate_limit_reset) {
                    error = GeminiError::RateLimitedWithRetry(Duration::from_secs(seconds));
                }
                error
            }
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
        {
            let cached_token = self.cached_token.read().await;
            if let Some(ref token_data) = *cached_token
                && !token_data.is_expired()
            {
                return Ok(token_data.token.clone());
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_http_error_rate_limit_with_retry_after() {
        let config = GeminiConfig {
            auth_method: AuthMethod::ApiKey("test_key".to_string()),
            project_id: "test_project".to_string(),
            location: "us-central1".to_string(),
            api_endpoint: None,
        };

        let client = GeminiClient {
            client: reqwest::Client::new(),
            api_key: Some("test_key".to_string()),
            base_url: DEFAULT_API_BASE.to_string(),
            config,
            cached_token: Arc::new(RwLock::new(None)),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            max_retries: MAX_RETRIES,
        };

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("retry-after", "60".parse().unwrap());

        let error = client.map_http_error(429, "Rate limited".to_string(), headers);

        match error {
            GeminiError::RateLimitedWithRetry(duration) => {
                assert_eq!(duration.as_secs(), 60);
            }
            _ => panic!("Expected RateLimitedWithRetry error"),
        }
    }

    #[test]
    fn test_map_http_error_rate_limit_with_ratelimit_reset() {
        let config = GeminiConfig {
            auth_method: AuthMethod::ApiKey("test_key".to_string()),
            project_id: "test_project".to_string(),
            location: "us-central1".to_string(),
            api_endpoint: None,
        };

        let client = GeminiClient {
            client: reqwest::Client::new(),
            api_key: Some("test_key".to_string()),
            base_url: DEFAULT_API_BASE.to_string(),
            config,
            cached_token: Arc::new(RwLock::new(None)),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            max_retries: MAX_RETRIES,
        };

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-reset", "30".parse().unwrap());

        let error = client.map_http_error(429, "Rate limited".to_string(), headers);

        match error {
            GeminiError::RateLimitedWithRetry(duration) => {
                assert_eq!(duration.as_secs(), 30);
            }
            _ => panic!("Expected RateLimitedWithRetry error"),
        }
    }

    #[test]
    fn test_map_http_error_rate_limit_without_headers() {
        let config = GeminiConfig {
            auth_method: AuthMethod::ApiKey("test_key".to_string()),
            project_id: "test_project".to_string(),
            location: "us-central1".to_string(),
            api_endpoint: None,
        };

        let client = GeminiClient {
            client: reqwest::Client::new(),
            api_key: Some("test_key".to_string()),
            base_url: DEFAULT_API_BASE.to_string(),
            config,
            cached_token: Arc::new(RwLock::new(None)),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            max_retries: MAX_RETRIES,
        };

        let headers = reqwest::header::HeaderMap::new();

        let error = client.map_http_error(429, "Rate limited".to_string(), headers);

        match error {
            GeminiError::RateLimited => {}
            _ => panic!("Expected RateLimited error without retry duration"),
        }
    }

    #[test]
    fn test_extract_retry_delay() {
        let config = GeminiConfig {
            auth_method: AuthMethod::ApiKey("test_key".to_string()),
            project_id: "test_project".to_string(),
            location: "us-central1".to_string(),
            api_endpoint: None,
        };

        let client = GeminiClient {
            client: reqwest::Client::new(),
            api_key: Some("test_key".to_string()),
            base_url: DEFAULT_API_BASE.to_string(),
            config,
            cached_token: Arc::new(RwLock::new(None)),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            max_retries: MAX_RETRIES,
        };

        let error_with_retry = GeminiError::RateLimitedWithRetry(Duration::from_secs(45));
        assert_eq!(
            client.extract_retry_delay(&error_with_retry),
            Some(Duration::from_secs(45))
        );

        let error_without_retry = GeminiError::RateLimited;
        assert_eq!(client.extract_retry_delay(&error_without_retry), None);

        let other_error = GeminiError::Timeout;
        assert_eq!(client.extract_retry_delay(&other_error), None);
    }
}
