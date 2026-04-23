#[cfg(test)]
mod tests;

#[cfg(test)]
mod vm_test;

pub use structured_agent_bytecode_compiler::{BytecodeCompiler, BytecodeRefs, compile_metadata};
pub use structured_agent_il::{BytecodeRef, CompiledFunction, Instruction};
pub use structured_agent_vm::{BytecodeFunctionExpr, VM};
