use std::collections::HashMap;

use crate::{IlAnalyzer, IlWarning};
use structured_agent_il::{BytecodeRef, Instruction};

pub struct CallArityAnalyzer {
    function_arities: HashMap<String, usize>,
}

impl CallArityAnalyzer {
    pub fn new(function_arities: HashMap<String, usize>) -> Self {
        Self { function_arities }
    }
}

impl IlAnalyzer for CallArityAnalyzer {
    fn name(&self) -> &str {
        "call-arity"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let mut warnings = Vec::new();

        for (index, instruction) in function.instructions.iter().enumerate() {
            let call_parts = match instruction {
                Instruction::CallBytecode {
                    function_name,
                    params,
                    ..
                }
                | Instruction::CallExternal {
                    function_name,
                    params,
                    ..
                } => Some((function_name, params)),
                _ => None,
            };
            if let Some((function_name, params)) = call_parts {
                let key = function_name.to_string();
                if let Some(&expected) = self.function_arities.get(&key) {
                    let got = params.len();
                    if got != expected {
                        warnings.push(IlWarning::CallArityMismatch {
                            function_name: key,
                            expected,
                            got,
                            instruction_index: index,
                        });
                    }
                }
            }
        }

        warnings
    }
}
