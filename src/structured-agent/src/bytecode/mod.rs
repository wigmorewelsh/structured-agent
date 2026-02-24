mod builder;
mod compiler;
mod instruction;
mod vm;

#[cfg(test)]
mod tests;

pub use builder::InstructionBuilder;
pub use compiler::{BytecodeCompiler, CompiledFunction};
pub use instruction::Instruction;
pub use vm::VM;
