use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};

use arrow::array::{Array, NullArray};

use crate::expression::{ExpressionResult, ExpressionValue};
use crate::runtime_value::RuntimeValue;
use crate::symbols::DefinitionPath;

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
    // this is for tests only
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

#[derive(Debug)]
pub struct ActorMessage {
    pub function_name: DefinitionPath,
    pub args: Vec<ExpressionResult>,
    pub reply: oneshot::Sender<Result<ExpressionValue, String>>,
}

#[derive(Clone)]
pub struct ActorRef {
    pub module_path: DefinitionPath,
    pub actor_id: String,
    mailbox: mpsc::Sender<ActorMessage>,
}

impl ActorRef {
    pub async fn call(
        &self,
        function_name: DefinitionPath,
        args: Vec<ExpressionResult>,
    ) -> Result<ExpressionValue, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.mailbox
            .send(ActorMessage {
                function_name,
                args,
                reply: reply_tx,
            })
            .await
            .map_err(|_| "actor mailbox closed".to_string())?;
        reply_rx
            .await
            .map_err(|_| "actor reply channel closed".to_string())?
    }
}

impl std::fmt::Debug for ActorRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorRef")
            .field("module_path", &self.module_path)
            .field("actor_id", &self.actor_id)
            .finish()
    }
}

impl RuntimeValue for ActorRef {
    fn type_name(&self) -> &str {
        "ActorRef"
    }

    fn format_for_llm(&self) -> String {
        format!("ActorRef({}:{})", self.module_path, self.actor_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<ActorRef>()
            .map(|r| r.actor_id == self.actor_id)
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        Arc::new(NullArray::new(1))
    }
}

pub type ActorMailboxReceiver = mpsc::Receiver<ActorMessage>;

pub struct ActorRegistry {
    inner: std::sync::Mutex<HashMap<String, mpsc::Sender<ActorMessage>>>,
}

impl ActorRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: std::sync::Mutex::new(HashMap::new()),
        })
    }

    pub fn get_or_create<F>(&self, key: String, module_path: DefinitionPath, create: F) -> ActorRef
    where
        F: FnOnce(ActorMailboxReceiver),
    {
        let mut lock = self.inner.lock().expect("actor registry lock poisoned");
        if let Some(sender) = lock.get(&key) {
            ActorRef {
                actor_id: key,
                module_path,
                mailbox: sender.clone(),
            }
        } else {
            let (tx, rx) = mpsc::channel(32);
            lock.insert(key.clone(), tx.clone());
            create(rx);
            ActorRef {
                actor_id: key,
                module_path,
                mailbox: tx,
            }
        }
    }
}
