use structured_agent::gemini::types::GenerationConfig;
use structured_agent::gemini::{ChatMessage, GeminiConfig, ModelName};
use tokio;

#[tokio::test]
async fn test_gemini_config_creation() {
    let config = GeminiConfig::new("test-project".to_string(), "europe-west9".to_string())
        .with_api_key_auth("test-api-key".to_string());

    assert_eq!(config.project_id, "test-project");
    assert_eq!(config.location, "europe-west9");
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn test_gemini_config_validation() {
    let empty_config = GeminiConfig::new("".to_string(), "europe-west9".to_string());
    assert!(empty_config.validate().is_err());

    let valid_config = GeminiConfig::new("valid-project".to_string(), "europe-west9".to_string())
        .with_api_key_auth("test-api-key".to_string());
    assert!(valid_config.validate().is_ok());
}

#[tokio::test]
async fn test_chat_message_creation() {
    let user_msg = ChatMessage::user("Hello, Gemini!");
    assert_eq!(user_msg.content, "Hello, Gemini!");

    let model_msg = ChatMessage::model("Hello! How can I help you?");
    assert_eq!(model_msg.content, "Hello! How can I help you?");

    let system_msg = ChatMessage::system("You are a helpful assistant.");
    assert_eq!(system_msg.content, "You are a helpful assistant.");
}

#[tokio::test]
async fn test_generation_config() {
    let config = GenerationConfig::new()
        .with_temperature(0.7)
        .with_top_k(40)
        .with_top_p(0.9)
        .with_max_output_tokens(1024);

    assert_eq!(config.temperature, Some(0.7));
    assert_eq!(config.top_k, Some(40));
    assert_eq!(config.top_p, Some(0.9));
    assert_eq!(config.max_output_tokens, Some(1024));
}

#[tokio::test]
async fn test_model_name_formatting() {
    let model = ModelName::Gemini25Flash;
    assert_eq!(model.as_str(), "gemini-2.5-flash");

    let full_name = model.full_name("test-project", "europe-west9");
    assert_eq!(
        full_name,
        "projects/test-project/locations/europe-west9/publishers/google/models/gemini-2.5-flash"
    );
}

#[tokio::test]
async fn test_temperature_clamping() {
    let config = GenerationConfig::new().with_temperature(3.0);
    assert_eq!(config.temperature, Some(2.0));

    let config2 = GenerationConfig::new().with_temperature(-1.0);
    assert_eq!(config2.temperature, Some(0.0));
}
