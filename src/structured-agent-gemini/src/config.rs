use serde::{Deserialize, Serialize};
use std::env;

const DEFAULT_PROJECT_ID: &str = "gemini-api";
const DEFAULT_LOCATION: &str = "global";
const DEFAULT_VERTEX_LOCATION: &str = "europe-west9";
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
        if let Ok(api_key) = env::var("GEMINI_API_KEY") {
            Ok(Self {
                project_id: DEFAULT_PROJECT_ID.to_string(),
                location: DEFAULT_LOCATION.to_string(),
                api_endpoint: Some(DEFAULT_API_ENDPOINT.to_string()),
                auth_method: AuthMethod::ApiKey(api_key),
            })
        } else {
            let project_id = env::var("VERTEX_AI_PROJECT")
                .or_else(|_| env::var("GOOGLE_CLOUD_PROJECT"))
                .or_else(|_| env::var("GCP_PROJECT"))
                .or_else(|_| {
                    std::process::Command::new("gcloud")
                        .args(["config", "get-value", "project"])
                        .output()
                        .ok()
                        .and_then(|output| {
                            if output.status.success() {
                                let project = String::from_utf8(output.stdout)
                                    .ok()?
                                    .trim()
                                    .to_string();
                                if !project.is_empty() && project != "(unset)" {
                                    Some(project)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "gcloud config not found"))
                })
                .map_err(|_| "For ADC auth, set VERTEX_AI_PROJECT, GOOGLE_CLOUD_PROJECT or GCP_PROJECT environment variable, or ensure gcloud is configured with a default project")?;

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
