pub mod acp;
pub use structured_agent_analysis as analysis;
pub use structured_agent_ast::ast;
pub mod bytecode;
pub mod cli;
pub mod compiler;
pub mod diagnostics;
pub mod expressions;

pub use structured_agent_gemini as gemini;
pub use structured_agent_il_analysis as il_analysis;
pub mod mcp;
pub mod runtime;
pub use structured_agent_typecheck as typecheck;
pub use structured_agent_typed_ast as typed_ast;
pub mod types;

#[cfg(test)]
mod integration_test;

#[cfg(test)]
mod test_doc;
