pub mod builder;
pub mod bytecode_ref;
mod instruction;
pub mod module_trait;
pub mod native_function_def;
pub mod slot;

pub use bytecode_ref::{BytecodeRef, CompiledFunction};
pub use instruction::Instruction;
pub use module_trait::{Module, NativeImplDecl, NativeTraitDecl, NativeTraitFnDecl};
pub use native_function_def::NativeFunctionDef;
pub use slot::{Slot, SlotInfo, SlotKind, SlotTable};
