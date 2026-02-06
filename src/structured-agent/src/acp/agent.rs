use crate::cli::config::Config;
use crate::compiler::CompilationUnit;
use crate::runtime::{ExprResult, Runtime, RuntimeError, load_program};
use agent_client_protocol as acp;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use super::functions::ReceiveFunction;
use super::tracing::SessionTracingLayer;

pub struct Agent {
    runtime: Rc<Runtime>,
    program: CompilationUnit,
    session_id: acp::SessionId,
    update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    prompt_tx: mpsc::UnboundedSender<PromptMessage>,
    task_handle: Option<tokio::task::JoinHandle<Result<ExprResult, AgentError>>>,
}

#[derive(Debug)]
pub struct PromptMessage {
    pub content: String,
    pub response_tx: oneshot::Sender<()>,
}

#[derive(Debug)]
pub enum AgentError {
    RuntimeError(RuntimeError),
    Cancelled,
    AlreadyRunning,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::RuntimeError(e) => write!(f, "Runtime error: {}", e),
            AgentError::Cancelled => write!(f, "Session cancelled"),
            AgentError::AlreadyRunning => write!(f, "Session is already running"),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<RuntimeError> for AgentError {
    fn from(error: RuntimeError) -> Self {
        AgentError::RuntimeError(error)
    }
}

impl Agent {
    pub async fn from_config(
        config: &Config,
        program_source: &crate::cli::config::ProgramSource,
        session_id: acp::SessionId,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<Self, String> {
        let program =
            load_program(program_source).map_err(|e| format!("Failed to load program: {}", e))?;

        let (prompt_tx, prompt_rx) = mpsc::unbounded_channel();

        let mut builder = Runtime::builder(program.clone())
            .with_native_function(Arc::new(ReceiveFunction::new(prompt_rx)));

        let runtime = builder.from_config(config).await?;

        Ok(Self {
            runtime: Rc::new(runtime),
            program,
            session_id,
            update_tx,
            prompt_tx,
            task_handle: None,
        })
    }

    pub fn new(
        program: CompilationUnit,
        session_id: acp::SessionId,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        let (prompt_tx, prompt_rx) = mpsc::unbounded_channel();

        let runtime = Runtime::builder(program.clone())
            .with_native_function(Arc::new(ReceiveFunction::new(prompt_rx)))
            .build();

        Self {
            runtime: Rc::new(runtime),
            program,
            session_id,
            update_tx,
            prompt_tx,
            task_handle: None,
        }
    }

    pub fn start(&mut self) -> Result<(), AgentError> {
        if self.task_handle.is_some() {
            return Err(AgentError::AlreadyRunning);
        }

        let runtime = self.runtime.clone();
        let session_id = self.session_id.clone();
        let update_tx = self.update_tx.clone();

        let handle = tokio::task::spawn_local(async move {
            let tracing_layer = SessionTracingLayer::new(session_id.clone(), update_tx.clone());

            let session_span = tracing::info_span!(
                "session",
                session_id = %session_id.0
            );

            let _guard = tracing_subscriber::registry()
                .with(tracing_layer)
                .set_default();

            let _span_guard = session_span.enter();

            runtime.run().await.map_err(Into::into)
        });

        self.task_handle = Some(handle);
        Ok(())
    }

    pub async fn send_prompt(&self, content: String) -> Result<(), AgentError> {
        let (response_tx, response_rx) = oneshot::channel();
        let message = PromptMessage {
            content,
            response_tx,
        };

        self.prompt_tx
            .send(message)
            .map_err(|_| AgentError::Cancelled)?;

        response_rx.await.map_err(|_| AgentError::Cancelled)?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn prompt_channel(&self) -> mpsc::UnboundedSender<PromptMessage> {
        self.prompt_tx.clone()
    }

    #[allow(dead_code)]
    pub async fn wait(mut self) -> Result<ExprResult, AgentError> {
        if let Some(handle) = self.task_handle.take() {
            handle.await.map_err(|_| AgentError::Cancelled)?
        } else {
            Err(AgentError::Cancelled)
        }
    }
}
