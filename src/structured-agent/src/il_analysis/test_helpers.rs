use std::collections::HashMap;

use crate::bytecode::{BytecodeRef, Instruction};
use crate::types::Type;

pub(super) fn make_function(instructions: Vec<Instruction>) -> BytecodeRef {
    BytecodeRef {
        instructions,
        labels: HashMap::new(),
        parameters: vec![],
        return_type: Type::Unit,
        documentation: None,
    }
}
