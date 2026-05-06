use agent_client_protocol as acp;
use agent_client_protocol::Client as _;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::{debug, error, warn};

use super::session::AcpSession;
use crate::cli::config::Config;

const ACP_INTERNAL_ERROR: i32 = -32603;

pub struct AcpServer {
    config: Arc<Config>,
    session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    next_session_id: AtomicU64,
    agents: Arc<Mutex<HashMap<String, Arc<Mutex<AcpSession>>>>>,
    agent_tasks: Arc<std::sync::Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
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
            next_session_id: AtomicU64::new(0),
            agents: Arc::new(Mutex::new(HashMap::new())),
            agent_tasks: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    async fn spawn_agent_creation(&self, session_id: acp::SessionId, working_dir: Option<String>) {
        debug!("Spawning session creation for: {}", session_id.0);

        let config = self.config.clone();
        let program_source = self.config.program_source.clone();
        let session_id_clone = session_id.clone();
        let update_tx = self.session_update_tx.clone();
        let agents = self.agents.clone();
        let agent_tasks = self.agent_tasks.clone();

        let handle = super::AGENT_RUNTIME.spawn(Self::create_and_start_agent(
            config,
            program_source,
            session_id_clone,
            update_tx,
            agents,
            agent_tasks,
            working_dir,
        ));

        self.agent_tasks
            .lock()
            .unwrap()
            .insert(session_id.0.to_string(), handle);
    }

    async fn create_and_start_agent(
        config: Arc<Config>,
        program_source: crate::cli::config::ProgramSource,
        session_id: acp::SessionId,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
        agents: Arc<Mutex<HashMap<String, Arc<Mutex<AcpSession>>>>>,
        agent_tasks: Arc<std::sync::Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
        working_dir: Option<String>,
    ) {
        let result: Result<(), String> = async {
            let mut session = AcpSession::from_config(
                &config,
                &program_source,
                session_id.clone(),
                update_tx,
                working_dir,
            )
            .await?;

            session.start().map_err(|e| e.to_string())?;

            agents
                .lock()
                .await
                .insert(session_id.0.to_string(), Arc::new(Mutex::new(session)));

            Ok(())
        }
        .await;

        if let Err(e) = result {
            error!("Failed to create/start session for {}: {}", session_id.0, e);
        }

        agent_tasks
            .lock()
            .unwrap()
            .remove(&session_id.0.to_string());
    }
}

impl Drop for AcpServer {
    fn drop(&mut self) {
        for (_id, handle) in self.agent_tasks.lock().unwrap().drain() {
            handle.abort();
        }
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
        args: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse, acp::Error> {
        let working_dir = args.cwd.to_string_lossy().into_owned();
        if let Err(e) = std::env::set_current_dir(&args.cwd) {
            error!("Failed to set working directory to {:?}: {}", args.cwd, e);
        }
        let session_id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let session_id = acp::SessionId::new(session_id.to_string());

        debug!("New session request: {}", session_id.0);

        self.spawn_agent_creation(session_id.clone(), Some(working_dir))
            .await;

        debug!("Session {} creation initiated", session_id.0);

        send_available_commands(&session_id, &self.session_update_tx).await?;

        Ok(acp::NewSessionResponse::new(session_id.0.to_string()))
    }

    async fn prompt(&self, args: acp::PromptRequest) -> Result<acp::PromptResponse, acp::Error> {
        debug!("Prompt request for session: {}", args.session_id.0);
        let prompt_content = format!("{:?}", args.prompt);
        debug!("Prompt content: {}", prompt_content);

        if prompt_content.contains("/reload") {
            debug!("Reload command for session: {}", args.session_id.0);

            let session_arc = {
                let agents = self.agents.lock().await;
                agents
                    .get(&args.session_id.0.to_string())
                    .cloned()
                    .ok_or_else(|| {
                        error!("Session not found: {}", args.session_id.0);
                        acp::Error::new(ACP_INTERNAL_ERROR, "Session not found")
                    })?
            };

            session_arc
                .lock()
                .await
                .reload_scripts()
                .await
                .map_err(|e| {
                    error!("Failed to reload scripts: {}", e);
                    acp::Error::new(ACP_INTERNAL_ERROR, format!("Reload failed: {}", e))
                })?;

            debug!("Scripts reloaded for session: {}", args.session_id.0);
            return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
        }

        let session_arc = {
            let agents = self.agents.lock().await;
            agents
                .get(&args.session_id.0.to_string())
                .cloned()
                .ok_or_else(|| {
                    error!("Session not found: {}", args.session_id.0);
                    acp::Error::new(ACP_INTERNAL_ERROR, "Session not found")
                })?
        };

        session_arc
            .lock()
            .await
            .send_prompt(prompt_content)
            .await
            .map_err(|e| {
                error!("Failed to send prompt: {}", e);
                acp::Error::new(ACP_INTERNAL_ERROR, format!("Send failed: {}", e))
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
