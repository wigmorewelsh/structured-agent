use crate::runtime::{
    Agent, AgentError, AgentId, AgentMessage, AgentMessageContent, ExpressionValue, Runtime,
};
use std::io::Write;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};

pub struct User {
    agent: Agent,
}

impl User {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        User {
            agent: Agent::new(runtime),
        }
    }

    pub async fn run(self) -> Result<ExpressionValue, AgentError> {
        let events_rx = self.agent.subscribe();
        let messagebox_tx = self.agent.messagebox_tx();
        let pending_input: Arc<Mutex<Option<oneshot::Sender<String>>>> = Arc::new(Mutex::new(None));
        let pending_input_clone = pending_input.clone();

        let event_handle = tokio::spawn(Self::run_event_loop(events_rx, pending_input.clone()));
        let stdin_handle = tokio::spawn(Self::run_stdin_loop(messagebox_tx, pending_input_clone));

        let result = self.agent.run().await;

        event_handle.abort();
        stdin_handle.abort();
        result
    }

    async fn run_event_loop(
        mut events_rx: broadcast::Receiver<AgentMessage>,
        pending_input: Arc<Mutex<Option<oneshot::Sender<String>>>>,
    ) {
        loop {
            match events_rx.recv().await {
                Ok(msg) => match msg.content {
                    AgentMessageContent::String(s) => {
                        println!("{}", s);
                    }
                    AgentMessageContent::RequestUserInput {
                        prompt,
                        response_channel,
                    } => {
                        if let Some(tx) = response_channel.lock().await.take() {
                            if !prompt.is_empty() {
                                print!("{}", prompt);
                            } else {
                                print!("> ");
                            }
                            std::io::stdout().flush().ok();
                            *pending_input.lock().await = Some(tx);
                        }
                    }
                    _ => {}
                },
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }

    async fn run_stdin_loop(
        messagebox_tx: mpsc::UnboundedSender<(AgentMessage, oneshot::Sender<()>)>,
        pending_input: Arc<Mutex<Option<oneshot::Sender<String>>>>,
    ) {
        let stdin = tokio::io::stdin();
        let reader = tokio::io::BufReader::new(stdin);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut pending = pending_input.lock().await;
            if let Some(tx) = pending.take() {
                drop(pending);
                tx.send(line).ok();
            } else {
                drop(pending);
                let (ack_tx, ack_rx) = oneshot::channel();
                let msg = AgentMessage {
                    source: AgentId("user".to_string()),
                    content: AgentMessageContent::String(line),
                };
                if messagebox_tx.send((msg, ack_tx)).is_err() {
                    break;
                }
                ack_rx.await.ok();
            }
        }
    }
}
