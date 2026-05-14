pub mod head;
pub mod tail;

pub use head::head_native_def;
pub use tail::tail_native_def;

pub struct UnstableModule;

impl ::structured_agent_il::Module for UnstableModule {
    fn name(&self) -> &str {
        "unstable"
    }

    fn native_functions(&self) -> Vec<::structured_agent_il::NativeFunctionDef> {
        vec![head_native_def(), tail_native_def()]
    }
}
