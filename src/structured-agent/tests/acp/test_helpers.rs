use agent_client_protocol as acp;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use structured_agent::acp::agent::{Agent, PromptMessage};
use structured_agent::cli::config::{Config, EngineType, Mode, ProgramSource};
use structured_agent::runtime::ExpressionValue;
use tokio::sync::{mpsc, oneshot};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_test_id() -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("test-{}", id)
}

pub struct TestAgent {
    pub agent: Agent,
    prompt_tx: mpsc::UnboundedSender<PromptMessage>,
    updates: Option<Arc<Mutex<Vec<String>>>>,
    _notification_task: tokio::task::JoinHandle<()>,
}

impl TestAgent {
    pub async fn from_program(program: &str) -> Self {
        let config = Config {
            program_source: ProgramSource::Inline(program.to_string()),
            engine: EngineType::Print,
            mcp_servers: vec![],
            with_default_functions: true,
            with_unstable_functions: false,
            mode: Mode::Acp,
        };

        Self::from_config(config).await
    }

    pub async fn from_config(config: Config) -> Self {
        Self::from_config_with_tracing(config, false).await
    }

    pub async fn with_tracing(program: &str) -> Self {
        let config = Config {
            program_source: ProgramSource::Inline(program.to_string()),
            engine: EngineType::Print,
            mcp_servers: vec![],
            with_default_functions: true,
            with_unstable_functions: false,
            mode: Mode::Acp,
        };

        Self::from_config_with_tracing(config, true).await
    }

    async fn from_config_with_tracing(config: Config, capture_tracing: bool) -> Self {
        let updates = if capture_tracing {
            Some(Arc::new(Mutex::new(Vec::new())))
        } else {
            None
        };
        let updates_clone = updates.clone();

        let (tx, mut rx) =
            mpsc::unbounded_channel::<(acp::SessionNotification, oneshot::Sender<()>)>();

        let notification_task = tokio::spawn(async move {
            while let Some((notification, response_tx)) = rx.recv().await {
                if let Some(ref updates) = updates_clone {
                    if let acp::SessionUpdate::AgentMessageChunk(chunk) = notification.update {
                        if let acp::ContentBlock::Text(text) = chunk.content {
                            updates.lock().unwrap().push(text.text);
                        }
                    }
                }
                response_tx.send(()).ok();
            }
        });

        let session_id = acp::SessionId::new(next_test_id());

        let mut agent = Agent::from_config(&config, &config.program_source, session_id, tx)
            .await
            .unwrap();

        agent.start().unwrap();

        let prompt_tx = agent.prompt_channel();

        Self {
            agent,
            prompt_tx,
            updates,
            _notification_task: notification_task,
        }
    }

    pub async fn send_prompt(&self, content: impl Into<String>) {
        let (response_tx, response_rx) = oneshot::channel();
        self.prompt_tx
            .send(PromptMessage {
                content: content.into(),
                response_tx,
            })
            .unwrap();

        response_rx.await.unwrap();
    }

    pub async fn wait(self) -> ExpressionValue {
        self.agent.wait().await.unwrap()
    }

    pub async fn wait_with_updates(self) -> (ExpressionValue, Vec<String>) {
        let result = self.agent.wait().await.unwrap();
        let updates = self
            .updates
            .map(|u| u.lock().unwrap().clone())
            .unwrap_or_default();
        (result, updates)
    }

    pub fn prompt_tx(&self) -> mpsc::UnboundedSender<PromptMessage> {
        self.prompt_tx.clone()
    }

    pub async fn reload(&mut self) {
        self.agent.reload_scripts().await.unwrap();
    }

    pub async fn try_reload(&mut self) -> Result<(), structured_agent::acp::agent::AgentError> {
        self.agent.reload_scripts().await
    }

    pub fn get_updates(&self) -> Vec<String> {
        self.updates
            .as_ref()
            .map(|u| u.lock().unwrap().clone())
            .unwrap_or_default()
    }

    pub fn assert_has_updates(&self) {
        let updates = self.get_updates();
        assert!(!updates.is_empty(), "Should have captured tracing updates");
    }

    pub fn assert_contains(&self, pattern: &str) {
        let all_updates = self.get_updates().join("\n");
        assert!(
            all_updates.contains(pattern),
            "Expected updates to contain '{}', but got: {}",
            pattern,
            all_updates
        );
    }
}

pub async fn run_local<F, Fut>(f: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let local_set = tokio::task::LocalSet::new();
    local_set.run_until(f()).await;
}
