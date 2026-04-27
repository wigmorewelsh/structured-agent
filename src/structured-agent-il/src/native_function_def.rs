use structured_agent_runtime::{Parameter, Type};

use crate::Instruction;

#[derive(Clone, Debug)]
pub struct NativeFunctionDef {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub type_params: Vec<String>,
    pub documentation: Option<String>,
    pub body: Vec<Instruction>,
}

impl NativeFunctionDef {
    pub fn new(
        name: String,
        parameters: Vec<Parameter>,
        return_type: Type,
        type_params: Vec<String>,
        documentation: Option<String>,
        body: Vec<Instruction>,
    ) -> Self {
        Self {
            name,
            parameters,
            return_type,
            type_params,
            documentation,
            body,
        }
    }
}
