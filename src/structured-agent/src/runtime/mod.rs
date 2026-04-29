pub mod actor;
mod engine;
mod types;

#[cfg(test)]
mod scoping_test;

#[cfg(test)]
mod function_call_test;

#[cfg(test)]
mod boolean_test;

#[cfg(test)]
mod control_flow_test;

#[cfg(test)]
mod signature_mismatch_test;

pub use actor::{Agent, AgentError, AgentHandle, AgentId, AgentMessage, AgentMessageContent};
pub use engine::{Runtime, RuntimeError};
pub use structured_agent_interpreter_runtime::{ActionEvent, Context, RuntimeService};
pub use types::{ExpressionParameter, ExpressionResult, ExpressionValue};

#[cfg(test)]
pub fn program(source: &str) -> crate::cli::config::ProgramSource {
    crate::cli::config::ProgramSource::Inline(source.to_string())
}
