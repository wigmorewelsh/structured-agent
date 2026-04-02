use crate::cli::config::{Config, ProgramSource};
use crate::runtime::{
    Agent, AgentError, AgentId, AgentMessage, AgentMessageContent, ExpressionValue, Runtime,
    RuntimeError,
};
use agent_client_protocol as acp;
use std::collections::HashMap;
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
    event_task_handle: Option<tokio::task::JoinHandle<Result<(), ()>>>,
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
        let update_tx_for_task = self.update_tx.clone();
        let session_id_for_task = self.session_id.clone();

        self.event_task_handle = Some(tokio::spawn(Self::run_event_loop(
            events_rx,
            update_tx_clone,
            self.session_id.clone(),
            pending_input,
        )));

        let runtime_arc = self.agent.runtime.clone();
        let handle = self.agent.handle.clone();

        self.task_handle = Some(AGENT_RUNTIME.spawn(async move {
            let result = runtime_arc
                .run_with_handle(handle)
                .await
                .map_err(AgentError::from);

            if let Err(ref e) = result {
                let (tx, _) = oneshot::channel();
                let notification = acp::SessionNotification::new(
                    session_id_for_task,
                    acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                        acp::ContentBlock::Text(acp::TextContent::new(e.to_string())),
                    )),
                );
                update_tx_for_task.send((notification, tx)).ok();
            }
            result
        }));

        debug!("Session {} started", self.session_id.0);
        Ok(())
    }

    async fn run_event_loop(
        mut events_rx: broadcast::Receiver<AgentMessage>,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
        session_id: acp::SessionId,
        pending_input: Arc<Mutex<Option<oneshot::Sender<String>>>>,
    ) -> Result<(), ()> {
        loop {
            match events_rx.recv().await {
                Ok(msg) => match msg.content {
                    AgentMessageContent::String(s) => {
                        Self::handle_string(s, &session_id, &update_tx).await?;
                    }
                    AgentMessageContent::RequestUserInput {
                        prompt,
                        response_channel,
                    } => {
                        Self::handle_request_user_input(
                            prompt,
                            response_channel,
                            &session_id,
                            &update_tx,
                            &pending_input,
                        )
                        .await?;
                    }
                    AgentMessageContent::ToolCallStarted {
                        tool_name,
                        call_id,
                        params,
                    } => {
                        Self::handle_tool_call_started(
                            tool_name,
                            call_id,
                            params,
                            &session_id,
                            &update_tx,
                        )
                        .await?;
                    }
                    AgentMessageContent::ToolCallFinished {
                        tool_name,
                        call_id,
                        result,
                    } => {
                        Self::handle_tool_call_finished(
                            tool_name,
                            call_id,
                            result,
                            &session_id,
                            &update_tx,
                        )
                        .await?;
                    }
                    _ => {}
                },
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }

    async fn handle_string(
        s: String,
        session_id: &acp::SessionId,
        update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<(), ()> {
        let (tx, rx) = oneshot::channel();
        let notification = acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(s),
            ))),
        );
        update_tx.send((notification, tx)).map_err(|_| ())?;
        rx.await.ok();
        Ok(())
    }

    async fn handle_request_user_input(
        prompt: String,
        response_channel: Arc<Mutex<Option<oneshot::Sender<String>>>>,
        session_id: &acp::SessionId,
        update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
        pending_input: &Arc<Mutex<Option<oneshot::Sender<String>>>>,
    ) -> Result<(), ()> {
        if let Some(tx) = response_channel.lock().await.take() {
            *pending_input.lock().await = Some(tx);
        }
        let (ack_tx, ack_rx) = oneshot::channel();
        let notification = acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(if prompt.is_empty() {
                    "> ".to_string()
                } else {
                    prompt
                }),
            ))),
        );
        update_tx.send((notification, ack_tx)).map_err(|_| ())?;
        ack_rx.await.ok();
        Ok(())
    }

    async fn handle_tool_call_started(
        tool_name: String,
        call_id: String,
        params: HashMap<String, ExpressionValue>,
        session_id: &acp::SessionId,
        update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<(), ()> {
        let (tx, rx) = oneshot::channel();
        let raw_input = serde_json::Value::Object(
            params
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v.value_string())))
                .collect(),
        );
        let notification = acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::ToolCall(
                acp::ToolCall::new(call_id, tool_name)
                    .status(acp::ToolCallStatus::InProgress)
                    .raw_input(raw_input),
            ),
        );
        update_tx.send((notification, tx)).map_err(|_| ())?;
        rx.await.ok();
        Ok(())
    }

    async fn handle_tool_call_finished(
        tool_name: String,
        call_id: String,
        result: ExpressionValue,
        session_id: &acp::SessionId,
        update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<(), ()> {
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
        update_tx.send((notification, tx)).map_err(|_| ())?;
        rx.await.ok();
        Ok(())
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
