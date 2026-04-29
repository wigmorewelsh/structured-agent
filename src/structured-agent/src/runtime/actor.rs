use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot};

pub use structured_agent_runtime::{
    AgentError, AgentHandle, AgentId, AgentMessage, AgentMessageContent,
};

use crate::bytecode::{VM, VMOutcome, VMState};
use crate::runtime::{Context, ExpressionResult, ExpressionValue, Runtime, RuntimeService};
use structured_agent_runtime::{ActorMailboxReceiver, ActorMessage};

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

pub async fn actor_loop(
    mut mailbox: ActorMailboxReceiver,
    mut context: Context,
    runtime: Arc<dyn RuntimeService>,
) {
    let mut pending: VecDeque<(VMState, oneshot::Sender<Result<ExpressionValue, String>>)> =
        VecDeque::new();
    while let Some(msg) = mailbox.recv().await {
        let vm = VM::new(runtime.clone());
        let body = match runtime.get_bytecode_ref(&msg.function_name) {
            Some(b) => b,
            None => {
                let _ = msg
                    .reply
                    .send(Err(format!("Function not found: {}", msg.function_name)));
                continue;
            }
        };
        let mut frame: Vec<Option<ExpressionResult>> = vec![None; body.slot_table.len()];
        for (i, arg) in msg.args.iter().enumerate() {
            frame[i + 1] = Some(arg.clone());
        }
        let handle = context.agent_handle().clone();
        match vm.execute_outcome(&body.instructions, context, frame).await {
            Err(e) => {
                context = Context::with_runtime_and_handle(runtime.clone(), handle);
                let _ = msg.reply.send(Err(e));
            }
            Ok(VMOutcome::Complete(new_ctx, result)) => {
                context = new_ctx;
                let _ = msg.reply.send(Ok(result.value));
            }
            Ok(VMOutcome::Yielded(state)) => {
                let yielded_handle = state.agent_handle().clone();
                context = Context::with_runtime_and_handle(runtime.clone(), yielded_handle);
                pending.push_back((state, msg.reply));
            }
        }
        let mut still_pending = VecDeque::new();
        while let Some((parked_state, reply)) = pending.pop_front() {
            match vm.resume_outcome(parked_state).await {
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
                Ok(VMOutcome::Complete(new_ctx, result)) => {
                    context = new_ctx;
                    let _ = reply.send(Ok(result.value));
                }
                Ok(VMOutcome::Yielded(new_state)) => {
                    still_pending.push_back((new_state, reply));
                }
            }
        }
        pending = still_pending;
    }
}
