use agent_client_protocol as acp;
use async_trait::async_trait;
use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use crate::cli::config::Config;
use crate::runtime::{ExprResult, Runtime, load_program};

const ACP_INTERNAL_ERROR: i32 = -32603;

pub struct StructuredAgent {
    config: Arc<Config>,
    session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    next_session_id: Cell<u64>,
}

impl StructuredAgent {
    pub fn new(
        config: Config,
        session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            session_update_tx,
            next_session_id: Cell::new(0),
        }
    }

    async fn execute_program(&self, _session_id: &acp::SessionId) -> Result<String, acp::Error> {
        let program = load_program(&self.config.program_source).map_err(|e| {
            acp::Error::new(ACP_INTERNAL_ERROR, format!("Failed to read file: {}", e))
        })?;

        let runtime = Runtime::builder()
            .from_config(&self.config)
            .await
            .map_err(|e| acp::Error::new(ACP_INTERNAL_ERROR, e))?;

        match runtime.run(&program).await {
            Ok(result) => Ok(format_result(&result)),
            Err(e) => Err(acp::Error::new(
                ACP_INTERNAL_ERROR,
                format!("Runtime error: {}", e),
            )),
        }
    }

    async fn send_update(
        &self,
        session_id: acp::SessionId,
        content: String,
    ) -> Result<(), acp::Error> {
        let (tx, rx) = oneshot::channel();
        self.session_update_tx
            .send((
                acp::SessionNotification::new(
                    session_id,
                    acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                        acp::ContentBlock::Text(acp::TextContent::new(content)),
                    )),
                ),
                tx,
            ))
            .map_err(|_| acp::Error::internal_error())?;
        rx.await.map_err(|_| acp::Error::internal_error())?;
        Ok(())
    }
}

#[async_trait(?Send)]
impl acp::Agent for StructuredAgent {
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
        Ok(acp::NewSessionResponse::new(session_id.to_string()))
    }

    async fn prompt(&self, args: acp::PromptRequest) -> Result<acp::PromptResponse, acp::Error> {
        let result = self.execute_program(&args.session_id).await?;
        self.send_update(args.session_id, result).await?;

        Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
    }

    async fn cancel(&self, _args: acp::CancelNotification) -> Result<(), acp::Error> {
        Ok(())
    }
}

fn format_result(result: &ExprResult) -> String {
    match result {
        ExprResult::String(s) => s.clone(),
        ExprResult::Unit => "(no output)".to_string(),
        ExprResult::Boolean(b) => b.to_string(),
        ExprResult::List(list) => {
            use arrow::array::Array;
            format!("List[{}]", list.len())
        }
        ExprResult::Option(opt) => match opt {
            Some(inner) => format!("Some({})", format_result(inner)),
            None => "None".to_string(),
        },
    }
}
