#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    FunctionNotFound(String),
    InvalidArguments(String),
    ExecutionError(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::FunctionNotFound(s) => write!(f, "Function not found: {}", s),
            RuntimeError::InvalidArguments(s) => write!(f, "Invalid arguments: {}", s),
            RuntimeError::ExecutionError(s) => write!(f, "Execution error: {}", s),
        }
    }
}

impl std::error::Error for RuntimeError {}

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
            AgentError::Cancelled => write!(f, "Agent cancelled"),
            AgentError::AlreadyRunning => write!(f, "Agent already running"),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<RuntimeError> for AgentError {
    fn from(e: RuntimeError) -> Self {
        AgentError::RuntimeError(e)
    }
}
