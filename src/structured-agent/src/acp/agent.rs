use crate::cli::config::Config;
use crate::compiler::CompilationUnit;
use crate::runtime::{ExprResult, Runtime, RuntimeError, load_program};
use agent_client_protocol as acp;
use std::fs::OpenOptions;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use super::functions::ReceiveFunction;
use super::tracing::SessionTracingLayer;

pub struct Agent {
    runtime: Rc<Runtime>,
    program: CompilationUnit,
    program_source: Option<crate::cli::config::ProgramSource>,
    config: Option<Arc<Config>>,
    session_id: acp::SessionId,
    update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    prompt_tx: mpsc::UnboundedSender<PromptMessage>,
    task_handle: Option<tokio::task::JoinHandle<Result<ExprResult, AgentError>>>,
}

#[derive(Debug)]
pub struct PromptMessage {
    pub content: String,
    pub response_tx: oneshot::Sender<()>,
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
            AgentError::Cancelled => write!(f, "Session cancelled"),
            AgentError::AlreadyRunning => write!(f, "Session is already running"),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<RuntimeError> for AgentError {
    fn from(error: RuntimeError) -> Self {
        AgentError::RuntimeError(error)
    }
}

impl Agent {
    pub async fn from_config(
        config: &Config,
        program_source: &crate::cli::config::ProgramSource,
        session_id: acp::SessionId,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Result<Self, String> {
        debug!("Creating agent for session {}", session_id.0);

        let program = match load_program(program_source) {
            Ok(p) => {
                debug!("Program loaded successfully");
                p
            }
            Err(e) => {
                error!("Failed to load program: {}", e);
                return Err(format!("Failed to load program: {}", e));
            }
        };

        let (prompt_tx, prompt_rx) = mpsc::unbounded_channel();

        debug!("Building runtime");
        let runtime = match Runtime::builder(program.clone())
            .with_native_function(Arc::new(ReceiveFunction::new(prompt_rx)))
            .from_config(config)
            .await
        {
            Ok(r) => {
                debug!("Runtime built successfully");
                r
            }
            Err(e) => {
                error!("Failed to build runtime: {}", e);
                return Err(e);
            }
        };

        Ok(Self {
            runtime: Rc::new(runtime),
            program,
            program_source: Some(program_source.clone()),
            config: Some(Arc::new(config.clone())),
            session_id,
            update_tx,
            prompt_tx,
            task_handle: None,
        })
    }

    pub fn new(
        program: CompilationUnit,
        session_id: acp::SessionId,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        let (prompt_tx, prompt_rx) = mpsc::unbounded_channel();

        let runtime = Runtime::builder(program.clone())
            .with_native_function(Arc::new(ReceiveFunction::new(prompt_rx)))
            .build();

        Self {
            runtime: Rc::new(runtime),
            program,
            program_source: None,
            config: None,
            session_id,
            update_tx,
            prompt_tx,
            task_handle: None,
        }
    }

    pub fn start(&mut self) -> Result<(), AgentError> {
        if self.task_handle.is_some() {
            warn!("Agent already running");
            return Err(AgentError::AlreadyRunning);
        }

        debug!("Starting agent session {}", self.session_id.0);

        let runtime = self.runtime.clone();
        let session_id = self.session_id.clone();
        let update_tx = self.update_tx.clone();

        let handle = tokio::task::spawn_local(async move {
            debug!("Agent task spawned for session {}", session_id.0);

            let tracing_layer = SessionTracingLayer::new(session_id.clone(), update_tx.clone());

            let log_dir = dirs::home_dir()
                .map(|home| home.join(".structured-agent").join("acp-logs"))
                .unwrap_or_else(|| std::path::PathBuf::from("acp-logs"));

            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                error!("Failed to create log directory {:?}: {}", log_dir, e);
            }

            let log_path = log_dir.join(format!("session-{}.log", session_id.0));
            debug!("Logging to {:?}", log_path);

            let file_layer =
                if let Ok(file) = OpenOptions::new().create(true).append(true).open(&log_path) {
                    debug!("Log file opened successfully");
                    Some(
                        fmt::layer()
                            .with_writer(Arc::new(file))
                            .with_ansi(false)
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_line_number(true),
                    )
                } else {
                    error!("Failed to create log file at {:?}", log_path);
                    None
                };

            let session_span = tracing::info_span!(
                "session",
                session_id = %session_id.0
            );

            let env_filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

            let registry = tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_layer);

            let _guard = if let Some(file_layer) = file_layer {
                registry.with(file_layer).set_default()
            } else {
                registry.set_default()
            };

            let _span_guard = session_span.enter();

            debug!("Starting runtime execution");
            match runtime.run().await {
                Ok(result) => {
                    debug!("Runtime execution completed successfully");
                    debug!("Result: {:?}", result);
                    Ok(result)
                }
                Err(e) => {
                    error!("Runtime execution failed: {:?}", e);
                    Err(e.into())
                }
            }
        });

        self.task_handle = Some(handle);
        debug!("Agent started successfully");
        Ok(())
    }

    pub async fn send_prompt(&self, content: String) -> Result<(), AgentError> {
        let (response_tx, response_rx) = oneshot::channel();
        let message = PromptMessage {
            content,
            response_tx,
        };

        self.prompt_tx
            .send(message)
            .map_err(|_| AgentError::Cancelled)?;

        response_rx.await.map_err(|_| AgentError::Cancelled)?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn prompt_channel(&self) -> mpsc::UnboundedSender<PromptMessage> {
        self.prompt_tx.clone()
    }

    #[allow(dead_code)]
    pub async fn wait(mut self) -> Result<ExprResult, AgentError> {
        if let Some(handle) = self.task_handle.take() {
            handle.await.map_err(|_| AgentError::Cancelled)?
        } else {
            Err(AgentError::Cancelled)
        }
    }

    pub async fn reload_scripts(&mut self) -> Result<(), AgentError> {
        debug!("Reloading scripts for session {}", self.session_id.0);

        let program_source = self.program_source.as_ref().ok_or_else(|| {
            error!("Cannot reload scripts: no program source available");
            AgentError::RuntimeError(RuntimeError::ExecutionError(
                "Reload not available for this session".to_string(),
            ))
        })?;

        let config = self.config.as_ref().ok_or_else(|| {
            error!("Cannot reload scripts: no config available");
            AgentError::RuntimeError(RuntimeError::ExecutionError(
                "Reload not available for this session".to_string(),
            ))
        })?;

        let program = load_program(program_source).map_err(|e| {
            error!("Failed to reload program: {}", e);
            AgentError::RuntimeError(RuntimeError::ExecutionError(format!(
                "Failed to reload program: {}",
                e
            )))
        })?;

        let (prompt_tx, prompt_rx) = mpsc::unbounded_channel();

        let runtime = Runtime::builder(program.clone())
            .with_native_function(Arc::new(ReceiveFunction::new(prompt_rx)))
            .from_config(config)
            .await
            .map_err(|e| {
                error!("Failed to rebuild runtime: {}", e);
                AgentError::RuntimeError(RuntimeError::ExecutionError(e))
            })?;

        self.program = program;
        self.runtime = Rc::new(runtime);
        self.prompt_tx = prompt_tx;

        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }

        self.start()?;

        info!(
            "Scripts reloaded successfully for session {}",
            self.session_id.0
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::{Config, EngineType, Mode, ProgramSource};
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_reload_scripts_from_file() {
        let local_set = tokio::task::LocalSet::new();
        local_set
            .run_until(async {
                let mut temp_file = NamedTempFile::new().unwrap();
                writeln!(temp_file, "fn main() {{ }}").unwrap();
                temp_file.flush().unwrap();

                let file_path = temp_file.path().to_str().unwrap().to_string();

                let config = Config {
                    program_source: ProgramSource::File(file_path.clone()),
                    engine: EngineType::Print,
                    mcp_servers: vec![],
                    with_default_functions: false,
                    mode: Mode::Acp,
                };

                let (tx, mut rx) =
                    mpsc::unbounded_channel::<(acp::SessionNotification, oneshot::Sender<()>)>();
                tokio::spawn(async move {
                    while let Some((_notif, response_tx)) = rx.recv().await {
                        response_tx.send(()).ok();
                    }
                });

                let session_id = acp::SessionId::new("test-reload".to_string());
                let mut agent = Agent::from_config(&config, &config.program_source, session_id, tx)
                    .await
                    .unwrap();

                agent.start().unwrap();

                fs::write(temp_file.path(), "fn main() { }").unwrap();

                let result = agent.reload_scripts().await;
                assert!(result.is_ok(), "Reload should succeed");
            })
            .await;
    }

    #[tokio::test]
    async fn test_reload_scripts_without_source_fails() {
        let local_set = tokio::task::LocalSet::new();
        local_set
            .run_until(async {
                use crate::compiler::CompilationUnit;

                let program = CompilationUnit::from_string("fn main() { }".to_string());

                let (tx, mut rx) =
                    mpsc::unbounded_channel::<(acp::SessionNotification, oneshot::Sender<()>)>();
                tokio::spawn(async move {
                    while let Some((_notif, response_tx)) = rx.recv().await {
                        response_tx.send(()).ok();
                    }
                });

                let session_id = acp::SessionId::new("test-no-reload".to_string());
                let mut agent = Agent::new(program, session_id, tx);

                agent.start().unwrap();

                let result = agent.reload_scripts().await;
                assert!(result.is_err(), "Reload should fail without program source");
            })
            .await;
    }
}
