use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot};

pub use structured_agent_runtime::{
    AgentError, AgentHandle, AgentId, AgentMessage, AgentMessageContent,
};

use crate::runtime::{ExpressionValue, Runtime};

pub struct Agent {
    pub runtime: Arc<Runtime>,
    pub handle: AgentHandle,
    messagebox_tx: mpsc::UnboundedSender<(AgentMessage, oneshot::Sender<()>)>,
    task_handle: Option<tokio::task::JoinHandle<Result<ExpressionValue, AgentError>>>,
}

impl Agent {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        let (handle, messagebox_tx) = AgentHandle::pair();
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
