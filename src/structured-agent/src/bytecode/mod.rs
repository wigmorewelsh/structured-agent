mod builder;
mod compiler;
mod function_expr;
mod instruction;
mod vm;

#[cfg(test)]
mod tests;

pub use builder::InstructionBuilder;
pub use compiler::{BytecodeCompiler, CompiledFunction};
pub use function_expr::BytecodeFunctionExpr;
pub use instruction::Instruction;
pub use vm::VM;
