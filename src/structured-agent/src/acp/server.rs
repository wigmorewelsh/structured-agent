use agent_client_protocol as acp;
use agent_client_protocol::Client as _;
use async_trait::async_trait;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use super::agent::Agent;
use crate::cli::config::Config;
use crate::runtime::{Runtime, load_program};

const ACP_INTERNAL_ERROR: i32 = -32603;

pub struct AcpServer {
    config: Arc<Config>,
    session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    next_session_id: Cell<u64>,
    agents: RefCell<HashMap<String, Agent>>,
}

impl AcpServer {
    pub fn new(
        config: Config,
        session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            session_update_tx,
            next_session_id: Cell::new(0),
            agents: RefCell::new(HashMap::new()),
        }
    }

    async fn create_agent(&self, session_id: acp::SessionId) -> Result<Agent, acp::Error> {
        let program = load_program(&self.config.program_source).map_err(|e| {
            acp::Error::new(ACP_INTERNAL_ERROR, format!("Failed to read file: {}", e))
        })?;

        let runtime = Runtime::builder()
            .from_config(&self.config)
            .await
            .map_err(|e| acp::Error::new(ACP_INTERNAL_ERROR, e))?;

        let mut agent = Agent::new(
            runtime,
            program,
            session_id.clone(),
            self.session_update_tx.clone(),
        );

        agent
            .start()
            .map_err(|e| acp::Error::new(ACP_INTERNAL_ERROR, e.to_string()))?;

        Ok(agent)
    }
}

#[async_trait(?Send)]
impl acp::Agent for AcpServer {
    async fn initialize(
        &self,
        _args: acp::InitializeRequest,
    ) -> Result<acp::InitializeResponse, acp::Error> {
        Ok(
            acp::InitializeResponse::new(acp::ProtocolVersion::V1).agent_info(
                acp::Implementation::new("structured-agent", "0.1.0").title("Structured Agent"),
            ),
        )
    }

    async fn authenticate(
        &self,
        _args: acp::AuthenticateRequest,
    ) -> Result<acp::AuthenticateResponse, acp::Error> {
        Ok(acp::AuthenticateResponse::default())
    }

    async fn new_session(
        &self,
        _args: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse, acp::Error> {
        let session_id = self.next_session_id.get();
        self.next_session_id.set(session_id + 1);
        let session_id = acp::SessionId::new(session_id.to_string());

        let agent = self.create_agent(session_id.clone()).await?;

        self.agents
            .borrow_mut()
            .insert(session_id.0.to_string(), agent);

        Ok(acp::NewSessionResponse::new(session_id.0.to_string()))
    }

    async fn prompt(&self, args: acp::PromptRequest) -> Result<acp::PromptResponse, acp::Error> {
        let prompt_content = format!("{:?}", args.prompt);

        let agents = self.agents.borrow();
        let agent = agents
            .get(&args.session_id.0.to_string())
            .ok_or_else(|| acp::Error::new(ACP_INTERNAL_ERROR, "Agent not found"))?;

        agent
            .send_prompt(prompt_content)
            .await
            .map_err(|e| acp::Error::new(ACP_INTERNAL_ERROR, e.to_string()))?;

        Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
    }

    async fn cancel(&self, _args: acp::CancelNotification) -> Result<(), acp::Error> {
        Ok(())
    }
}

pub async fn run_acp_server(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let outgoing = tokio::io::stdout().compat_write();
    let incoming = tokio::io::stdin().compat();

    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            let (tx, mut rx) = mpsc::unbounded_channel();

            let (conn, handle_io) = acp::AgentSideConnection::new(
                AcpServer::new(config, tx),
                outgoing,
                incoming,
                |fut| {
                    tokio::task::spawn_local(fut);
                },
            );

            tokio::task::spawn_local(async move {
                while let Some((session_notification, tx)) = rx.recv().await {
                    let result = conn.session_notification(session_notification).await;
                    if let Err(e) = result {
                        eprintln!("Error sending session notification: {}", e);
                        break;
                    }
                    tx.send(()).ok();
                }
            });

            handle_io.await
        })
        .await?;

    Ok(())
}
