use serde::{Deserialize, Serialize};
use std::env;

// Constants for better maintainability
const DEFAULT_PROJECT_ID: &str = "gemini-api";
const DEFAULT_LOCATION: &str = "global";
const DEFAULT_VERTEX_LOCATION: &str = "us-central1";
const DEFAULT_API_ENDPOINT: &str = "https://generativelanguage.googleapis.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    ApiKey(String),
    ApplicationDefaultCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    pub project_id: String,
    pub location: String,
    pub api_endpoint: Option<String>,
    pub auth_method: AuthMethod,
}

impl GeminiConfig {
    pub fn new(project_id: String, location: String) -> Self {
        Self {
            project_id,
            location,
            api_endpoint: None,
            auth_method: AuthMethod::ApplicationDefaultCredentials,
        }
    }

    pub fn with_api_key(project_id: String, location: String, api_key: String) -> Self {
        Self {
            project_id,
            location,
            api_endpoint: None,
            auth_method: AuthMethod::ApiKey(api_key),
        }
    }

    pub fn with_api_key_auth(mut self, api_key: String) -> Self {
        self.auth_method = AuthMethod::ApiKey(api_key);
        self
    }

    pub fn with_adc_auth(mut self) -> Self {
        self.auth_method = AuthMethod::ApplicationDefaultCredentials;
        self
    }

    pub fn with_api_endpoint(mut self, endpoint: String) -> Self {
        self.api_endpoint = Some(endpoint);
        self
    }

    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        // Try API key first, fall back to ADC
        if let Ok(api_key) = env::var("GEMINI_API_KEY") {
            Ok(Self {
                project_id: DEFAULT_PROJECT_ID.to_string(),
                location: DEFAULT_LOCATION.to_string(),
                api_endpoint: Some(DEFAULT_API_ENDPOINT.to_string()),
                auth_method: AuthMethod::ApiKey(api_key),
            })
        } else {
            // Use Application Default Credentials with Vertex AI
            let project_id = env::var("VERTEX_AI_PROJECT")
                .or_else(|_| env::var("GOOGLE_CLOUD_PROJECT"))
                .or_else(|_| env::var("GCP_PROJECT"))
                .map_err(|_| "For ADC auth, set VERTEX_AI_PROJECT, GOOGLE_CLOUD_PROJECT or GCP_PROJECT environment variable")?;

            let location = env::var("VERTEX_AI_LOCATION")
                .or_else(|_| env::var("GOOGLE_CLOUD_REGION"))
                .or_else(|_| env::var("GCP_REGION"))
                .unwrap_or_else(|_| DEFAULT_VERTEX_LOCATION.to_string());

            Ok(Self {
                project_id,
                location,
                api_endpoint: None, // Use default Vertex AI endpoint
                auth_method: AuthMethod::ApplicationDefaultCredentials,
            })
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.project_id.is_empty() {
            return Err("Project ID cannot be empty".to_string());
        }
        if self.location.is_empty() {
            return Err("Location cannot be empty".to_string());
        }
        match &self.auth_method {
            AuthMethod::ApiKey(key) if key.is_empty() => {
                return Err("API key cannot be empty".to_string());
            }
            _ => {}
        }
        Ok(())
    }
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            project_id: DEFAULT_PROJECT_ID.to_string(),
            location: DEFAULT_LOCATION.to_string(),
            api_endpoint: Some(DEFAULT_API_ENDPOINT.to_string()),
            auth_method: AuthMethod::ApplicationDefaultCredentials,
        }
    }
}
