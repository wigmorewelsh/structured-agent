use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};

use super::{ExpressionValue, Runtime, RuntimeError};

#[derive(Debug, Clone)]
pub struct AgentId(pub String);

impl Default for AgentId {
    fn default() -> Self {
        AgentId("agent".to_string())
    }
}

#[derive(Clone, Debug)]
pub struct AgentMessage {
    pub source: AgentId,
    pub content: AgentMessageContent,
}

#[derive(Clone, Debug)]
pub enum AgentMessageContent {
    String(String),
    ToolCallStarted {
        tool_name: String,
        call_id: String,
        params: HashMap<String, ExpressionValue>,
    },
    ToolCallFinished {
        tool_name: String,
        call_id: String,
        result: ExpressionValue,
    },
    RequestUserInput {
        prompt: String,
        response_channel: Arc<Mutex<Option<oneshot::Sender<String>>>>,
    },
}

#[derive(Clone)]
pub struct AgentHandle {
    pub id: AgentId,
    events_tx: broadcast::Sender<AgentMessage>,
    messagebox_rx: Arc<Mutex<mpsc::UnboundedReceiver<(AgentMessage, oneshot::Sender<()>)>>>,
}

impl AgentHandle {
    pub fn detached() -> Self {
        let (events_tx, _) = broadcast::channel(16);
        let (_, messagebox_rx) = mpsc::unbounded_channel();
        Self {
            id: AgentId::default(),
            events_tx,
            messagebox_rx: Arc::new(Mutex::new(messagebox_rx)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentMessage> {
        self.events_tx.subscribe()
    }

    pub fn publish(&self, content: AgentMessageContent) {
        let msg = AgentMessage {
            source: self.id.clone(),
            content,
        };
        self.events_tx.send(msg).ok();
    }

    pub async fn publish_input_request(&self, prompt: String) -> Result<String, String> {
        let (tx, rx) = oneshot::channel::<String>();
        let response_channel = Arc::new(Mutex::new(Some(tx)));
        self.publish(AgentMessageContent::RequestUserInput {
            prompt,
            response_channel,
        });
        rx.await.map_err(|_| "Response channel closed".to_string())
    }

    pub async fn recv_message(&self) -> Result<(AgentMessage, oneshot::Sender<()>), String> {
        let mut rx = self.messagebox_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| "Messagebox closed".to_string())
    }

    pub async fn try_recv_message(&self) -> Option<(AgentMessage, oneshot::Sender<()>)> {
        let mut rx = self.messagebox_rx.lock().await;
        rx.try_recv().ok()
    }
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
            AgentError::Cancelled => write!(f, "Agent cancelled"),
            AgentError::AlreadyRunning => write!(f, "Agent already running"),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<RuntimeError> for AgentError {
    fn from(e: RuntimeError) -> Self {
        AgentError::RuntimeError(e)
    }
}

pub struct Agent {
    pub runtime: Arc<Runtime>,
    pub handle: AgentHandle,
    messagebox_tx: mpsc::UnboundedSender<(AgentMessage, oneshot::Sender<()>)>,
    task_handle: Option<tokio::task::JoinHandle<Result<ExpressionValue, AgentError>>>,
}

impl Agent {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        let (events_tx, _) = broadcast::channel(16);
        let (messagebox_tx, messagebox_rx) = mpsc::unbounded_channel();
        let handle = AgentHandle {
            id: AgentId::default(),
            events_tx,
            messagebox_rx: Arc::new(Mutex::new(messagebox_rx)),
        };
        Self {
            runtime,
            handle,
            messagebox_tx,
            task_handle: None,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentMessage> {
        self.handle.subscribe()
    }

    pub fn messagebox_tx(&self) -> mpsc::UnboundedSender<(AgentMessage, oneshot::Sender<()>)> {
        self.messagebox_tx.clone()
    }

    pub async fn run(&self) -> Result<ExpressionValue, AgentError> {
        self.runtime
            .run_with_handle(self.handle.clone())
            .await
            .map_err(AgentError::from)
    }

    pub fn start(&mut self) -> Result<(), AgentError> {
        if self.task_handle.is_some() {
            return Err(AgentError::AlreadyRunning);
        }
        let runtime = self.runtime.clone();
        let handle = self.handle.clone();
        self.task_handle = Some(tokio::spawn(async move {
            runtime
                .run_with_handle(handle)
                .await
                .map_err(AgentError::from)
        }));
        Ok(())
    }

    pub async fn wait(mut self) -> Result<ExpressionValue, AgentError> {
        if let Some(task) = self.task_handle.take() {
            task.await.map_err(|_| AgentError::Cancelled)?
        } else {
            Err(AgentError::Cancelled)
        }
    }
}
