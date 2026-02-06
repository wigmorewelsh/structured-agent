use agent_client_protocol as acp;
use agent_client_protocol::Client as _;
use async_trait::async_trait;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::{debug, error, info, warn};

use super::agent::Agent;
use crate::cli::config::Config;

const ACP_INTERNAL_ERROR: i32 = -32603;

pub struct AcpServer {
    config: Arc<Config>,
    session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    next_session_id: Cell<u64>,
    agents: RefCell<HashMap<String, Agent>>,
}

async fn send_available_commands(
    session_id: &acp::SessionId,
    session_update_tx: &mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
) -> Result<(), acp::Error> {
    let commands = vec![acp::AvailableCommand::new(
        "reload",
        "Reload scripts from disk for this session",
    )];

    let update =
        acp::SessionUpdate::AvailableCommandsUpdate(acp::AvailableCommandsUpdate::new(commands));

    let notification = acp::SessionNotification::new(session_id.clone(), update);

    let (response_tx, response_rx) = oneshot::channel();
    session_update_tx
        .send((notification, response_tx))
        .map_err(|_| {
            error!("Failed to send available commands notification");
            acp::Error::new(ACP_INTERNAL_ERROR, "Failed to send notification")
        })?;

    response_rx.await.map_err(|_| {
        error!("Failed to receive confirmation for available commands");
        acp::Error::new(ACP_INTERNAL_ERROR, "Notification failed")
    })?;

    debug!("Available commands sent for session: {}", session_id.0);
    Ok(())
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
        debug!("Creating agent for session: {}", session_id.0);

        let mut agent = Agent::from_config(
            &self.config,
            &self.config.program_source,
            session_id.clone(),
            self.session_update_tx.clone(),
        )
        .await
        .map_err(|e| {
            error!("Failed to create agent from config: {}", e);
            acp::Error::new(ACP_INTERNAL_ERROR, e)
        })?;

        agent.start().map_err(|e| {
            error!("Failed to start agent: {}", e);
            acp::Error::new(ACP_INTERNAL_ERROR, e.to_string())
        })?;

        debug!("Agent created and started successfully");
        Ok(agent)
    }
}

#[async_trait(?Send)]
impl acp::Agent for AcpServer {
    async fn initialize(
        &self,
        _args: acp::InitializeRequest,
    ) -> Result<acp::InitializeResponse, acp::Error> {
        debug!("ACP server initializing");
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
        debug!("Authentication request received");
        Ok(acp::AuthenticateResponse::default())
    }

    async fn new_session(
        &self,
        _args: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse, acp::Error> {
        let session_id = self.next_session_id.get();
        self.next_session_id.set(session_id + 1);
        let session_id = acp::SessionId::new(session_id.to_string());

        debug!("New session request: {}", session_id.0);

        let agent = self.create_agent(session_id.clone()).await?;

        self.agents
            .borrow_mut()
            .insert(session_id.0.to_string(), agent);

        debug!("Session {} created successfully", session_id.0);

        send_available_commands(&session_id, &self.session_update_tx).await?;

        Ok(acp::NewSessionResponse::new(session_id.0.to_string()))
    }

    async fn prompt(&self, args: acp::PromptRequest) -> Result<acp::PromptResponse, acp::Error> {
        debug!("Prompt request for session: {}", args.session_id.0);
        let prompt_content = format!("{:?}", args.prompt);
        debug!("Prompt content: {}", prompt_content);

        if prompt_content.contains("/reload") {
            info!("Reload command detected for session: {}", args.session_id.0);

            let mut agents = self.agents.borrow_mut();
            let agent = agents
                .get_mut(&args.session_id.0.to_string())
                .ok_or_else(|| {
                    error!("Agent not found for session: {}", args.session_id.0);
                    acp::Error::new(ACP_INTERNAL_ERROR, "Agent not found")
                })?;

            agent.reload_scripts().await.map_err(|e| {
                error!("Failed to reload scripts: {}", e);
                acp::Error::new(ACP_INTERNAL_ERROR, format!("Reload failed: {}", e))
            })?;

            info!(
                "Scripts reloaded successfully for session: {}",
                args.session_id.0
            );
            return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
        }

        let prompt_tx = {
            let agents = self.agents.borrow();
            let agent = agents.get(&args.session_id.0.to_string()).ok_or_else(|| {
                error!("Agent not found for session: {}", args.session_id.0);
                acp::Error::new(ACP_INTERNAL_ERROR, "Agent not found")
            })?;
            agent.prompt_channel()
        };

        let (response_tx, response_rx) = oneshot::channel();
        let message = super::agent::PromptMessage {
            content: prompt_content,
            response_tx,
        };

        prompt_tx.send(message).map_err(|_| {
            error!("Failed to send prompt to agent");
            acp::Error::new(ACP_INTERNAL_ERROR, "Agent cancelled")
        })?;

        debug!("Waiting for agent response");
        response_rx.await.map_err(|_| {
            error!("Agent cancelled or failed to respond");
            acp::Error::new(ACP_INTERNAL_ERROR, "Agent cancelled")
        })?;

        debug!("Prompt handled successfully");
        Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
    }

    async fn cancel(&self, _args: acp::CancelNotification) -> Result<(), acp::Error> {
        debug!("Cancel notification received");
        Ok(())
    }
}

pub async fn run_acp_server(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Starting ACP server");

    let outgoing = tokio::io::stdout().compat_write();
    let incoming = tokio::io::stdin().compat();

    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            let (tx, mut rx) = mpsc::unbounded_channel();

            debug!("Creating ACP connection");
            let (conn, handle_io) = acp::AgentSideConnection::new(
                AcpServer::new(config, tx),
                outgoing,
                incoming,
                |fut| {
                    tokio::task::spawn_local(fut);
                },
            );

            tokio::task::spawn_local(async move {
                debug!("Session notification handler started");
                while let Some((session_notification, tx)) = rx.recv().await {
                    let result = conn.session_notification(session_notification).await;
                    if let Err(e) = result {
                        error!("Error sending session notification: {}", e);
                        break;
                    }
                    tx.send(()).ok();
                }
                warn!("Session notification handler stopped");
            });

            debug!("ACP server ready, handling I/O");
            handle_io.await
        })
        .await?;

    debug!("ACP server stopped");
    Ok(())
}
