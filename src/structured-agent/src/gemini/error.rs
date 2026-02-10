use std::fmt;
use std::time::Duration;

#[derive(Debug)]
pub enum GeminiError {
    Configuration(String),
    Authentication(String),
    Network(String),
    ApiError { code: u32, message: String },
    InvalidInput(String),
    Timeout,
    RateLimited,
    RateLimitedWithRetry(Duration),
    QuotaExceeded,
    ModelNotFound(String),
    Serialization(String),
    Unknown(String),
}

impl fmt::Display for GeminiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeminiError::Configuration(msg) => write!(f, "Configuration error: {}", msg),
            GeminiError::Authentication(msg) => write!(f, "Authentication error: {}", msg),
            GeminiError::Network(msg) => write!(f, "Network error: {}", msg),
            GeminiError::ApiError { code, message } => {
                write!(f, "API error {}: {}", code, message)
            }
            GeminiError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            GeminiError::Timeout => write!(f, "Request timeout"),
            GeminiError::RateLimited => write!(f, "Rate limit exceeded"),
            GeminiError::RateLimitedWithRetry(duration) => {
                write!(
                    f,
                    "Rate limit exceeded, retry after {} seconds",
                    duration.as_secs()
                )
            }
            GeminiError::QuotaExceeded => write!(f, "Quota exceeded"),
            GeminiError::ModelNotFound(model) => write!(f, "Model not found: {}", model),
            GeminiError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            GeminiError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for GeminiError {}

impl From<serde_json::Error> for GeminiError {
    fn from(error: serde_json::Error) -> Self {
        GeminiError::Serialization(error.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for GeminiError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        GeminiError::Unknown(error.to_string())
    }
}

impl From<String> for GeminiError {
    fn from(error: String) -> Self {
        GeminiError::Unknown(error)
    }
}

pub type GeminiResult<T> = Result<T, GeminiError>;
