use std::sync::Arc;

use async_openai::{Client, config::Config};
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use secrecy::SecretString;
use structured_agent_interpreter_runtime::{
    Context, Event, ExpressionValue, LanguageEngine, ThinkingEvent,
};

use crate::{copilot_token::CopilotTokenProvider, engine::OpenAIEngine};

pub const COPILOT_BASE_URL: &str = "https://api.githubcopilot.com";

pub(crate) struct CopilotConfig {
    base_url: String,
    token: Arc<std::sync::RwLock<String>>,
    api_key: SecretString,
}

impl Clone for CopilotConfig {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            token: Arc::clone(&self.token),
            api_key: SecretString::from(String::new()),
        }
    }
}

impl Config for CopilotConfig {
    fn headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        let token = self.token.read().unwrap().clone();
        map.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );
        map.insert("Editor-Version", HeaderValue::from_static("vscode/1.85.0"));
        map.insert(
            "Copilot-Integration-Id",
            HeaderValue::from_static("vscode-chat"),
        );
        map
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn api_base(&self) -> &str {
        &self.base_url
    }

    fn api_key(&self) -> &SecretString {
        &self.api_key
    }

    fn query(&self) -> Vec<(&str, &str)> {
        vec![]
    }
}

pub struct CopilotEngine {
    pub(crate) inner: OpenAIEngine<CopilotConfig>,
    token_provider: CopilotTokenProvider,
    token: Arc<std::sync::RwLock<String>>,
}

impl CopilotEngine {
    pub fn new(pat: impl Into<String>, model: impl Into<String>) -> Self {
        Self::with_token_provider(CopilotTokenProvider::new(pat), COPILOT_BASE_URL, model)
    }

    pub(crate) fn with_token_provider(
        token_provider: CopilotTokenProvider,
        api_base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let token = Arc::new(std::sync::RwLock::new(String::new()));
        let config = CopilotConfig {
            base_url: api_base_url.into(),
            token: Arc::clone(&token),
            api_key: SecretString::from(String::new()),
        };
        let client = Client::with_config(config);
        let inner = OpenAIEngine::with_client(client, model);
        Self {
            inner,
            token_provider,
            token,
        }
    }
}

#[async_trait]
impl LanguageEngine for CopilotEngine {
    async fn request(
        &self,
        context: &Context,
        event: &dyn Event,
    ) -> Result<(ExpressionValue, Option<ThinkingEvent>), String> {
        let t = self
            .token_provider
            .get_token()
            .await
            .map_err(|e| format!("Copilot token error: {e:?}"))?;
        *self.token.write().unwrap() = t;
        self.inner.request(context, event).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::datatypes::DataType;
    use structured_agent_interpreter_runtime::{
        Context, DefinitionPath, ExecutableFunction, LanguageEngine, RuntimeService, Type,
        TypedEvent,
    };
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    struct MockRuntime;

    impl RuntimeService for MockRuntime {
        fn get_native_function(&self, _: &str) -> Option<Arc<dyn ExecutableFunction>> {
            None
        }
        fn get_bytecode_ref(
            &self,
            _: &DefinitionPath,
        ) -> Option<structured_agent_interpreter_runtime::BytecodeRef> {
            None
        }
        fn engine(&self) -> &dyn LanguageEngine {
            unimplemented!()
        }
        fn type_to_arrow_datatype(&self, _: &Type) -> DataType {
            DataType::Null
        }
        fn get_struct(&self, _: &DefinitionPath) -> Option<Vec<(String, Type)>> {
            None
        }
        fn get_struct_with_args(
            &self,
            _: &DefinitionPath,
            _: &[Type],
        ) -> Option<Vec<(String, Type)>> {
            None
        }
    }

    fn empty_context() -> Context {
        Context::with_runtime(Arc::new(MockRuntime))
    }

    #[test]
    fn copilot_engine_targets_copilot_base_url() {
        let engine = CopilotEngine::new("test-pat", "gpt-4o");
        assert_eq!(engine.inner.client.config().api_base(), COPILOT_BASE_URL);
    }

    #[tokio::test]
    async fn copilot_engine_calls_token_provider_on_each_request() {
        let mock_token_server = MockServer::start().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"token":"ghs_test","expires_at":{}}}"#,
                now + 1
            )))
            .expect(2u64)
            .mount(&mock_token_server)
            .await;

        let mock_chat_server = MockServer::start().await;

        let chat_response = r#"{
            "id": "chatcmpl-test",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "{\"value\": \"ok\"}"},
                "finish_reason": "stop"
            }],
            "created": 1234567890,
            "model": "gpt-4o",
            "object": "chat.completion"
        }"#;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(chat_response))
            .mount(&mock_chat_server)
            .await;

        let provider = CopilotTokenProvider::with_base_url("pat", mock_token_server.uri());
        let engine = CopilotEngine::with_token_provider(provider, mock_chat_server.uri(), "gpt-4o");

        let context = empty_context();
        let event = TypedEvent {
            return_type: Type::string(),
        };

        engine.request(&context, &event).await.unwrap();
        engine.request(&context, &event).await.unwrap();
    }
}
