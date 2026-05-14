use crate::context::Context;
use crate::traits::{ExecutableFunction, LanguageEngine};
use arrow::datatypes::DataType;
use std::sync::Arc;
use structured_agent_il::BytecodeRef;
use structured_agent_runtime::{ActorMailboxReceiver, ActorRegistry, DefinitionPath, Type};

pub trait RuntimeService: Send + Sync {
    fn get_native_function(&self, name: &str) -> Option<Arc<dyn ExecutableFunction>>;
    fn get_bytecode_ref(&self, name: &DefinitionPath) -> Option<BytecodeRef>;
    fn engine(&self) -> &dyn LanguageEngine;
    fn type_to_arrow_datatype(&self, ty: &Type) -> DataType;
    fn get_struct(&self, type_name: &DefinitionPath) -> Option<Vec<(String, Type)>>;
    fn get_struct_with_args(
        &self,
        type_name: &DefinitionPath,
        args: &[Type],
    ) -> Option<Vec<(String, Type)>>;

    fn actor_registry(&self) -> Arc<ActorRegistry> {
        unimplemented!("actor_registry not supported by this runtime")
    }

    fn spawn_actor(&self, _mailbox: ActorMailboxReceiver, _context: Context) {
        unimplemented!("spawn_actor not supported by this runtime")
    }
}
