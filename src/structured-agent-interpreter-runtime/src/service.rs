use crate::traits::{ExecutableFunction, LanguageEngine};
use arrow::datatypes::DataType;
use std::sync::Arc;
use structured_agent_runtime::{DefinitionPath, Type};

pub trait RuntimeService: Send + Sync {
    fn get_native_function(&self, name: &str) -> Option<Arc<dyn ExecutableFunction>>;
    fn get_bytecode_function(&self, name: &DefinitionPath) -> Option<Arc<dyn ExecutableFunction>>;
    fn engine(&self) -> &dyn LanguageEngine;
    fn type_to_arrow_datatype(&self, ty: &Type) -> DataType;
    fn get_struct(&self, type_name: &DefinitionPath) -> Option<Vec<(String, Type)>>;
    fn get_struct_with_args(
        &self,
        type_name: &DefinitionPath,
        args: &[Type],
    ) -> Option<Vec<(String, Type)>>;
}
