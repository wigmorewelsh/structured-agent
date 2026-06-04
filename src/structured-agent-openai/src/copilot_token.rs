use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

#[derive(Deserialize)]
struct CopilotTokenResponse {
    token: String,
    expires_at: u64,
}

pub(crate) struct CopilotTokenCache {
    token: String,
    expires_at: u64,
}

pub struct CopilotTokenProvider {
    pat: String,
    pub(crate) base_url: String,
    client: Client,
    pub(crate) cache: Mutex<Option<CopilotTokenCache>>,
}

#[derive(Debug)]
pub enum CopilotTokenError {
    HttpError(reqwest::Error),
    AuthError(u16),
    ParseError(String),
}

fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

impl CopilotTokenProvider {
    pub fn new(pat: impl Into<String>) -> Self {
        Self::with_base_url(pat, "https://api.github.com")
    }

    pub(crate) fn with_base_url(pat: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            pat: pat.into(),
            base_url: base_url.into(),
            client: Client::new(),
            cache: Mutex::new(None),
        }
    }

    pub async fn get_token(&self) -> Result<String, CopilotTokenError> {
        let mut cache = self.cache.lock().await;
        let now = current_unix_secs();

        if let Some(ref cached) = *cache {
            if now + 30 < cached.expires_at {
                return Ok(cached.token.clone());
            }
        }

        let url = format!("{}/copilot_internal/v2/token", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("token {}", self.pat))
            .header("User-Agent", "structured-agent")
            .send()
            .await
            .map_err(CopilotTokenError::HttpError)?;

        let status = response.status();
        if !status.is_success() {
            return Err(CopilotTokenError::AuthError(status.as_u16()));
        }

        let body = response
            .text()
            .await
            .map_err(CopilotTokenError::HttpError)?;

        let parsed: CopilotTokenResponse = serde_json::from_str(&body)
            .map_err(|e| CopilotTokenError::ParseError(e.to_string()))?;

        *cache = Some(CopilotTokenCache {
            token: parsed.token.clone(),
            expires_at: parsed.expires_at,
        });

        Ok(parsed.token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate, Times};

    #[tokio::test]
    async fn copilot_token_provider_fetches_token_from_github_api() {
        let mock_server = MockServer::start().await;
        let now = current_unix_secs();

        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"token":"ghs_abc","expires_at":{}}}"#,
                now + 3600
            )))
            .mount(&mock_server)
            .await;

        let provider = CopilotTokenProvider::with_base_url("pat", mock_server.uri());
        let result = provider.get_token().await;

        assert_eq!(result.unwrap(), "ghs_abc");
    }

    #[tokio::test]
    async fn copilot_token_provider_returns_cached_token_before_expiry() {
        let mock_server = MockServer::start().await;
        let now = current_unix_secs();

        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"token":"ghs_abc","expires_at":{}}}"#,
                now + 3600
            )))
            .expect(Times::from(1u64))
            .mount(&mock_server)
            .await;

        let provider = CopilotTokenProvider::with_base_url("pat", mock_server.uri());

        let first = provider.get_token().await.unwrap();
        let second = provider.get_token().await.unwrap();

        assert_eq!(first, "ghs_abc");
        assert_eq!(second, "ghs_abc");
    }

    #[tokio::test]
    async fn copilot_token_provider_refreshes_token_after_expiry() {
        let mock_server = MockServer::start().await;
        let now = current_unix_secs();

        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"token":"new_token","expires_at":{}}}"#,
                now + 3600
            )))
            .expect(Times::from(1u64))
            .mount(&mock_server)
            .await;

        let provider = CopilotTokenProvider::with_base_url("pat", mock_server.uri());

        {
            let mut cache = provider.cache.lock().await;
            *cache = Some(CopilotTokenCache {
                token: "old_token".to_string(),
                expires_at: now - 1,
            });
        }

        let result = provider.get_token().await.unwrap();
        assert_eq!(result, "new_token");
    }

    #[tokio::test]
    async fn copilot_token_provider_returns_error_on_non_200_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let provider = CopilotTokenProvider::with_base_url("pat", mock_server.uri());
        let result = provider.get_token().await;

        assert!(matches!(result, Err(CopilotTokenError::AuthError(401))));
    }

    #[tokio::test]
    async fn copilot_token_provider_returns_error_on_malformed_json() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"unexpected_field":true}"#),
            )
            .mount(&mock_server)
            .await;

        let provider = CopilotTokenProvider::with_base_url("pat", mock_server.uri());
        let result = provider.get_token().await;

        assert!(matches!(result, Err(CopilotTokenError::ParseError(_))));
    }
}
