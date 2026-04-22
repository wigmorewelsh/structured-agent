pub mod acp;
pub use structured_agent_analysis as analysis;
pub use structured_agent_ast::ast;
pub mod bytecode;
pub mod cli;
pub mod compiler;
pub mod diagnostics;
pub mod expressions;

pub mod gemini;
pub mod il_analysis;
pub mod mcp;
pub mod runtime;
pub mod typecheck;
pub mod typed_ast;
pub mod types;

#[cfg(test)]
mod test_doc;
