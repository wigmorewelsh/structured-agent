use crate::cli::config::{Config, ProgramSource};
use crate::runtime::{
    Agent, AgentError, AgentId, AgentMessage, AgentMessageContent, ExpressionValue, Runtime,
    RuntimeError,
};
use agent_client_protocol as acp;
use base64::{Engine as _, engine::general_purpose::STANDARD};
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
        working_dir: Option<String>,
    ) -> Result<Self, String> {
        debug!("Creating session for {}", session_id.0);

        let mut acp_config = config.clone();
        let mut builder = Runtime::builder(program_source.clone());
        if let Some(ref dir) = working_dir {
            builder = builder.with_mcp_working_dir(dir.clone());
        }
        let runtime = builder.with_config(&acp_config).await?;

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
                    AgentMessageContent::Thinking { content } => {
                        Self::handle_thinking(content, &session_id, &update_tx).await?;
                    }
                },
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }

    async fn send_notification(
        update: acp::SessionUpdate,
        session_id: &acp::SessionId,
        update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<(), ()> {
        let (tx, rx) = oneshot::channel();
        let notification = acp::SessionNotification::new(session_id.clone(), update);
        update_tx.send((notification, tx)).map_err(|_| ())?;
        rx.await.ok();
        Ok(())
    }

    async fn handle_string(
        s: String,
        session_id: &acp::SessionId,
        update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<(), ()> {
        Self::send_notification(
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(s),
            ))),
            session_id,
            update_tx,
        )
        .await
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
        let raw_input = serde_json::Value::Object(
            params
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v.value_string())))
                .collect(),
        );
        Self::send_notification(
            acp::SessionUpdate::ToolCall(
                acp::ToolCall::new(call_id, tool_name)
                    .status(acp::ToolCallStatus::InProgress)
                    .raw_input(raw_input),
            ),
            session_id,
            update_tx,
        )
        .await
    }

    async fn handle_tool_call_finished(
        tool_name: String,
        call_id: String,
        result: ExpressionValue,
        session_id: &acp::SessionId,
        update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<(), ()> {
        let content = vec![acp::ToolCallContent::from(
            expression_value_to_content_block(&result),
        )];
        Self::send_notification(
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                call_id,
                acp::ToolCallUpdateFields::new()
                    .status(acp::ToolCallStatus::Completed)
                    .title(tool_name)
                    .content(content),
            )),
            session_id,
            update_tx,
        )
        .await
    }

    async fn handle_thinking(
        content: String,
        session_id: &acp::SessionId,
        update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<(), ()> {
        let call_id = structured_agent_runtime::next_call_id();
        Self::send_notification(
            acp::SessionUpdate::ToolCall(
                acp::ToolCall::new(call_id.clone(), "thinking")
                    .status(acp::ToolCallStatus::InProgress),
            ),
            session_id,
            update_tx,
        )
        .await?;
        let thinking_content = vec![acp::ToolCallContent::from(acp::ContentBlock::Text(
            acp::TextContent::new(content),
        ))];
        Self::send_notification(
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                call_id,
                acp::ToolCallUpdateFields::new()
                    .status(acp::ToolCallStatus::Completed)
                    .title("thinking")
                    .content(thinking_content),
            )),
            session_id,
            update_tx,
        )
        .await
    }

    pub async fn send_prompt(&self, content: String) -> Result<(), AgentError> {
        debug!("send_prompt: checking pending_input");
        let mut pending = self.pending_input.lock().await;
        if let Some(tx) = pending.take() {
            debug!("send_prompt: routing via pending_input channel");
            drop(pending);
            tx.send(content).map_err(|_| AgentError::Cancelled)?;
            return Ok(());
        }
        drop(pending);

        debug!("send_prompt: sending to messagebox");
        let (ack_tx, ack_rx) = oneshot::channel();
        let msg = AgentMessage {
            source: AgentId("client".to_string()),
            content: AgentMessageContent::String(content),
        };
        self.agent
            .messagebox_tx()
            .send((msg, ack_tx))
            .map_err(|_| AgentError::Cancelled)?;
        debug!("send_prompt: awaiting ack");
        let result = ack_rx.await.map_err(|_| AgentError::Cancelled);
        debug!("send_prompt: ack received, result ok={}", result.is_ok());
        result
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
        let current_dir = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        let mut builder = Runtime::builder(self.program_source.clone());
        if let Some(ref dir) = current_dir {
            builder = builder.with_mcp_working_dir(dir.clone());
        }
        let runtime = builder.with_config(&acp_config).await.map_err(|e| {
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

pub fn expression_value_to_content_block(value: &ExpressionValue) -> acp::ContentBlock {
    if let Ok(img) = value.as_image() {
        let b64 = STANDARD.encode(&img.data);
        return acp::ContentBlock::Image(acp::ImageContent::new(b64, img.mime_type.clone()));
    }
    if let Ok(aud) = value.as_audio() {
        let b64 = STANDARD.encode(&aud.data);
        return acp::ContentBlock::Audio(acp::AudioContent::new(b64, aud.mime_type.clone()));
    }
    if let Ok(link) = value.as_link() {
        let display_name = link.name.as_deref().unwrap_or(&link.uri).to_string();
        return acp::ContentBlock::ResourceLink(acp::ResourceLink::new(
            display_name,
            link.uri.clone(),
        ));
    }
    acp::ContentBlock::Text(acp::TextContent::new(value.value_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn handle_tool_call_finished_with_image_produces_image_block() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let session_id = acp::SessionId::new("test-session");
        let value = ExpressionValue::image("image/png", b"abc".to_vec());
        let (result, notification) = tokio::join!(
            AcpSession::handle_tool_call_finished(
                "tool".to_string(),
                "call-1".to_string(),
                value,
                &session_id,
                &tx,
            ),
            async {
                let (n, ack) = rx.recv().await.unwrap();
                let _ = ack.send(());
                n
            }
        );
        result.unwrap();
        let acp::SessionUpdate::ToolCallUpdate(update) = notification.update else {
            panic!("Expected ToolCallUpdate");
        };
        let content = update.fields.content.unwrap();
        let acp::ToolCallContent::Content(inner) = &content[0] else {
            panic!("Expected Content variant");
        };
        let acp::ContentBlock::Image(img) = &inner.content else {
            panic!("Expected Image block");
        };
        assert_eq!(img.mime_type, "image/png");
        let decoded = STANDARD.decode(&img.data).unwrap();
        assert_eq!(decoded, b"abc");
    }

    #[tokio::test]
    async fn handle_tool_call_finished_with_string_produces_text_block() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let session_id = acp::SessionId::new("test-session");
        let value = ExpressionValue::string("hello");
        let (result, notification) = tokio::join!(
            AcpSession::handle_tool_call_finished(
                "tool".to_string(),
                "call-2".to_string(),
                value,
                &session_id,
                &tx,
            ),
            async {
                let (n, ack) = rx.recv().await.unwrap();
                let _ = ack.send(());
                n
            }
        );
        result.unwrap();
        let acp::SessionUpdate::ToolCallUpdate(update) = notification.update else {
            panic!("Expected ToolCallUpdate");
        };
        let content = update.fields.content.unwrap();
        let acp::ToolCallContent::Content(inner) = &content[0] else {
            panic!("Expected Content variant");
        };
        let acp::ContentBlock::Text(text) = &inner.content else {
            panic!("Expected Text block");
        };
        assert_eq!(text.text, "hello");
    }

    #[test]
    fn image_value_converts_to_acp_image_block() {
        let value = ExpressionValue::image("image/png", b"abc".to_vec());
        let block = expression_value_to_content_block(&value);
        let acp::ContentBlock::Image(img) = block else {
            panic!("Expected Image block");
        };
        assert_eq!(img.mime_type, "image/png");
        let decoded = STANDARD.decode(&img.data).unwrap();
        assert_eq!(decoded, b"abc");
    }

    #[test]
    fn audio_value_converts_to_acp_audio_block() {
        let value = ExpressionValue::audio("audio/wav", b"xyz".to_vec());
        let block = expression_value_to_content_block(&value);
        let acp::ContentBlock::Audio(aud) = block else {
            panic!("Expected Audio block");
        };
        assert_eq!(aud.mime_type, "audio/wav");
        let decoded = STANDARD.decode(&aud.data).unwrap();
        assert_eq!(decoded, b"xyz");
    }

    #[test]
    fn link_value_converts_to_acp_resource_link() {
        let value =
            ExpressionValue::link("https://example.com/logo.png", Some("logo.png".to_string()));
        let block = expression_value_to_content_block(&value);
        let acp::ContentBlock::ResourceLink(link) = block else {
            panic!("Expected ResourceLink block");
        };
        assert_eq!(link.uri, "https://example.com/logo.png");
        assert_eq!(link.name, "logo.png");
    }
}
