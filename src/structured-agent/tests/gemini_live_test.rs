#[cfg(test)]
mod gemini_live_tests {
    use structured_agent::gemini::{
        ChatMessage, ChatRequest, GeminiClient, GeminiConfig, GenerationConfig, ModelName,
    };
    use tokio;

    #[tokio::test]
    #[ignore] // Use cargo test -- --ignored to run this test
    async fn test_live_gemini_simple_chat() {
        let client = match GeminiClient::from_env().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("Failed to create client: {}", e);
                eprintln!("Ensure you're authenticated with gcloud or have GEMINI_API_KEY set");
                panic!("Authentication failed");
            }
        };

        let response = client
            .simple_chat("What is 2+2? Respond with just the number.")
            .await;

        match response {
            Ok(content) => {
                assert!(!content.is_empty(), "Response should not be empty");
                assert!(
                    content.contains("4"),
                    "Response should contain the answer 4"
                );
                println!("✓ Simple chat test passed: {}", content);
            }
            Err(e) => {
                panic!("Simple chat failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_gemini_structured_chat() {
        let client = match GeminiClient::from_env().await {
            Ok(client) => client,
            Err(e) => {
                panic!("Failed to create client: {}", e);
            }
        };

        let messages = vec![
            ChatMessage::system("You are a helpful assistant. Keep responses short."),
            ChatMessage::user("Name one programming language used for systems programming."),
        ];

        let generation_config = GenerationConfig::new()
            .with_temperature(0.1)
            .with_max_output_tokens(50);

        let request = ChatRequest::new(messages, ModelName::Gemini25Flash)
            .with_generation_config(generation_config);

        let response = client.chat(request).await;

        match response {
            Ok(response) => {
                assert!(
                    !response.candidates.is_empty(),
                    "Should have at least one candidate"
                );

                if let Some(content) = response.first_content() {
                    assert!(!content.is_empty(), "Response content should not be empty");
                    println!("✓ Structured chat test passed: {}", content);
                } else {
                    panic!("No response content received");
                }

                if let Some(usage) = response.usage_metadata {
                    assert!(usage.total_token_count.is_some(), "Should have token count");
                    println!("✓ Token usage reported: {:?}", usage.total_token_count);
                }
            }
            Err(e) => {
                panic!("Structured chat failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_gemini_conversation() {
        let client = match GeminiClient::from_env().await {
            Ok(client) => client,
            Err(e) => {
                panic!("Failed to create client: {}", e);
            }
        };

        let mut conversation = vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("What is the capital of Japan?"),
        ];

        // First message
        let request = ChatRequest::new(conversation.clone(), ModelName::Gemini25Flash);
        let response = client.chat(request).await;

        let first_response = match response {
            Ok(response) => {
                if let Some(content) = response.first_content() {
                    assert!(
                        content.to_lowercase().contains("tokyo"),
                        "Response should mention Tokyo: {}",
                        content
                    );
                    println!("✓ First response: {}", content);
                    content.to_string()
                } else {
                    panic!("No content in first response");
                }
            }
            Err(e) => {
                panic!("First request failed: {}", e);
            }
        };

        // Continue conversation
        conversation.push(ChatMessage::model(first_response));
        conversation.push(ChatMessage::user("What about the population?"));

        let follow_up_request = ChatRequest::new(conversation, ModelName::Gemini25Flash);
        let follow_up_response = client.chat(follow_up_request).await;

        match follow_up_response {
            Ok(response) => {
                if let Some(content) = response.first_content() {
                    assert!(
                        !content.is_empty(),
                        "Follow-up response should not be empty"
                    );
                    println!("✓ Follow-up response: {}", content);
                } else {
                    panic!("No content in follow-up response");
                }
            }
            Err(e) => {
                panic!("Follow-up request failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_gemini_different_models() {
        let client = match GeminiClient::from_env().await {
            Ok(client) => client,
            Err(e) => {
                panic!("Failed to create client: {}", e);
            }
        };

        let models = vec![ModelName::Gemini25Flash, ModelName::Gemini25Pro];
        let prompt = "What is machine learning in one sentence?";

        for model in models {
            let messages = vec![ChatMessage::user(prompt)];
            let request = ChatRequest::new(messages, model.clone());

            let response = client.chat(request).await;

            match response {
                Ok(response) => {
                    if let Some(content) = response.first_content() {
                        assert!(
                            !content.is_empty(),
                            "Response from {} should not be empty",
                            model.as_str()
                        );
                        println!(
                            "✓ Model {} responded: {}",
                            model.as_str(),
                            content.chars().take(100).collect::<String>()
                        );
                    } else {
                        panic!("No content from model {}", model.as_str());
                    }
                }
                Err(e) => {
                    println!(
                        "⚠ Model {} failed (might not be available): {}",
                        model.as_str(),
                        e
                    );
                    // Don't panic here as some models might not be available
                }
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_gemini_error_handling() {
        let client = match GeminiClient::from_env().await {
            Ok(client) => client,
            Err(e) => {
                panic!("Failed to create client: {}", e);
            }
        };

        // Test with empty message (should handle gracefully)
        let response = client.simple_chat("").await;

        match response {
            Ok(content) => {
                println!("✓ Empty message handled: {}", content);
            }
            Err(e) => {
                println!("✓ Empty message properly rejected: {}", e);
                // This is expected behavior
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_gemini_authentication_method() {
        let config = match GeminiConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                panic!("Failed to create config: {}", e);
            }
        };

        let client = match GeminiClient::new(config).await {
            Ok(client) => client,
            Err(e) => {
                panic!("Failed to create client: {}", e);
            }
        };

        // Test which authentication method is being used
        if client.is_using_adc() {
            println!("✓ Using Application Default Credentials");
        } else if client.is_using_api_key() {
            println!("✓ Using API Key authentication");
        } else {
            panic!("Unknown authentication method");
        }

        // Verify it actually works
        let response = client.simple_chat("Hello").await;
        match response {
            Ok(_) => println!("✓ Authentication method works"),
            Err(e) => panic!("Authentication failed: {}", e),
        }
    }
}
