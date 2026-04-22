use std::collections::HashMap;

use structured_agent_il::{BytecodeRef, Instruction};
use structured_agent_runtime::Type;

pub fn make_function(instructions: Vec<Instruction>) -> BytecodeRef {
    BytecodeRef {
        instructions,
        labels: HashMap::new(),
        parameters: vec![],
        return_type: Type::unit(),
        documentation: None,
    }
}
