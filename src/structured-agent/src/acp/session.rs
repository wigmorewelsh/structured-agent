use crate::cli::config::{Config, ProgramSource};
use crate::runtime::{
    Agent, AgentError, AgentId, AgentMessage, AgentMessageContent, ExpressionValue, Runtime,
    RuntimeError,
};
use agent_client_protocol as acp;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tracing::{debug, error, warn};

use super::AGENT_RUNTIME;

pub struct AcpSession {
    agent: Agent,
    program_source: ProgramSource,
    config: Option<Arc<Config>>,
    session_id: acp::SessionId,
    update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    pending_input: Arc<Mutex<Option<oneshot::Sender<String>>>>,
    task_handle: Option<tokio::task::JoinHandle<Result<ExpressionValue, AgentError>>>,
    event_task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl AcpSession {
    pub async fn from_config(
        config: &Config,
        program_source: &ProgramSource,
        session_id: acp::SessionId,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<Self, String> {
        debug!("Creating session for {}", session_id.0);

        let mut acp_config = config.clone();
        acp_config.with_acp_functions = true;
        let runtime = Runtime::builder(program_source.clone())
            .with_config(&acp_config)
            .await?;

        let agent = Agent::new(Arc::new(runtime));

        Ok(Self {
            agent,
            program_source: program_source.clone(),
            config: Some(Arc::new(config.clone())),
            session_id,
            update_tx,
            pending_input: Arc::new(Mutex::new(None)),
            task_handle: None,
            event_task_handle: None,
        })
    }

    #[allow(dead_code)]
    pub fn new(
        program_source: ProgramSource,
        session_id: acp::SessionId,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        let runtime = Runtime::builder(program_source.clone()).build();
        let agent = Agent::new(Arc::new(runtime));

        Self {
            agent,
            program_source,
            config: None,
            session_id,
            update_tx,
            pending_input: Arc::new(Mutex::new(None)),
            task_handle: None,
            event_task_handle: None,
        }
    }

    pub fn start(&mut self) -> Result<(), AgentError> {
        if self.task_handle.is_some() {
            warn!("Session already running");
            return Err(AgentError::AlreadyRunning);
        }

        debug!("Starting session {}", self.session_id.0);

        let events_rx = self.agent.subscribe();
        let pending_input = self.pending_input.clone();
        let update_tx_clone = self.update_tx.clone();

        self.event_task_handle = Some(tokio::spawn(Self::run_event_loop(
            events_rx,
            update_tx_clone,
            self.session_id.clone(),
            pending_input,
        )));

        let runtime_arc = self.agent.runtime.clone();
        let handle = self.agent.handle.clone();

        self.task_handle = Some(AGENT_RUNTIME.spawn(async move {
            runtime_arc
                .run_with_handle(handle)
                .await
                .map_err(AgentError::from)
        }));

        debug!("Session {} started", self.session_id.0);
        Ok(())
    }

    async fn run_event_loop(
        mut events_rx: broadcast::Receiver<AgentMessage>,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
        session_id: acp::SessionId,
        pending_input: Arc<Mutex<Option<oneshot::Sender<String>>>>,
    ) {
        loop {
            match events_rx.recv().await {
                Ok(msg) => match msg.content {
                    AgentMessageContent::String(s) => {
                        let (tx, rx) = oneshot::channel();
                        let notification = acp::SessionNotification::new(
                            session_id.clone(),
                            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                                acp::ContentBlock::Text(acp::TextContent::new(s)),
                            )),
                        );
                        if update_tx.send((notification, tx)).is_err() {
                            break;
                        }
                        rx.await.ok();
                    }
                    AgentMessageContent::RequestUserInput {
                        prompt,
                        response_channel,
                    } => {
                        if let Some(tx) = response_channel.lock().await.take() {
                            *pending_input.lock().await = Some(tx);
                        }
                        let (ack_tx, ack_rx) = oneshot::channel();
                        let notification = acp::SessionNotification::new(
                            session_id.clone(),
                            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                                acp::ContentBlock::Text(acp::TextContent::new(
                                    if prompt.is_empty() {
                                        "> ".to_string()
                                    } else {
                                        prompt
                                    },
                                )),
                            )),
                        );
                        if update_tx.send((notification, ack_tx)).is_err() {
                            break;
                        }
                        ack_rx.await.ok();
                    }
                    AgentMessageContent::ToolCallStarted { tool_name, call_id } => {
                        let (tx, rx) = oneshot::channel();
                        let notification = acp::SessionNotification::new(
                            session_id.clone(),
                            acp::SessionUpdate::ToolCall(
                                acp::ToolCall::new(call_id, tool_name)
                                    .status(acp::ToolCallStatus::InProgress),
                            ),
                        );
                        if update_tx.send((notification, tx)).is_err() {
                            break;
                        }
                        rx.await.ok();
                    }
                    AgentMessageContent::ToolCallFinished {
                        tool_name,
                        call_id,
                        result,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        let content = vec![acp::ToolCallContent::from(acp::ContentBlock::Text(
                            acp::TextContent::new(result.value_string()),
                        ))];
                        let notification = acp::SessionNotification::new(
                            session_id.clone(),
                            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                                call_id,
                                acp::ToolCallUpdateFields::new()
                                    .status(acp::ToolCallStatus::Completed)
                                    .title(tool_name)
                                    .content(content),
                            )),
                        );
                        if update_tx.send((notification, tx)).is_err() {
                            break;
                        }
                        rx.await.ok();
                    }
                    _ => {}
                },
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }

    pub async fn send_prompt(&self, content: String) -> Result<(), AgentError> {
        let mut pending = self.pending_input.lock().await;
        if let Some(tx) = pending.take() {
            drop(pending);
            tx.send(content).map_err(|_| AgentError::Cancelled)?;
            return Ok(());
        }
        drop(pending);

        let (ack_tx, ack_rx) = oneshot::channel();
        let msg = AgentMessage {
            source: AgentId("client".to_string()),
            content: AgentMessageContent::String(content),
        };
        self.agent
            .messagebox_tx()
            .send((msg, ack_tx))
            .map_err(|_| AgentError::Cancelled)?;
        ack_rx.await.map_err(|_| AgentError::Cancelled)
    }

    pub async fn wait(mut self) -> Result<ExpressionValue, AgentError> {
        let event_task = self.event_task_handle.take();
        if let Some(task) = self.task_handle.take() {
            let result = task.await.map_err(|_| AgentError::Cancelled)?;
            drop(self);
            if let Some(t) = event_task {
                t.await.ok();
            }
            result
        } else {
            Err(AgentError::Cancelled)
        }
    }

    pub async fn reload_scripts(&mut self) -> Result<(), AgentError> {
        debug!("Reloading scripts for session {}", self.session_id.0);

        let config = self.config.as_ref().ok_or_else(|| {
            error!("Cannot reload scripts: no config available");
            AgentError::RuntimeError(RuntimeError::ExecutionError(
                "Reload not available for this session".to_string(),
            ))
        })?;

        let mut acp_config = (**config).clone();
        acp_config.with_acp_functions = true;
        let runtime = Runtime::builder(self.program_source.clone())
            .with_config(&acp_config)
            .await
            .map_err(|e| {
                error!("Failed to rebuild runtime: {}", e);
                AgentError::RuntimeError(RuntimeError::ExecutionError(e))
            })?;

        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.event_task_handle.take() {
            handle.abort();
        }

        self.agent = Agent::new(Arc::new(runtime));
        self.pending_input = Arc::new(Mutex::new(None));

        self.start()?;

        Ok(())
    }
}
