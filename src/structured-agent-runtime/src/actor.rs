use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};

use crate::expression::ExpressionValue;

type MessageboxRx = Arc<Mutex<mpsc::UnboundedReceiver<(AgentMessage, oneshot::Sender<()>)>>>;

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
    messagebox_rx: MessageboxRx,
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

    pub fn pair() -> (
        AgentHandle,
        mpsc::UnboundedSender<(AgentMessage, oneshot::Sender<()>)>,
    ) {
        let (events_tx, _) = broadcast::channel(16);
        let (messagebox_tx, messagebox_rx) = mpsc::unbounded_channel();
        let handle = AgentHandle {
            id: AgentId::default(),
            events_tx,
            messagebox_rx: Arc::new(Mutex::new(messagebox_rx)),
        };
        (handle, messagebox_tx)
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
