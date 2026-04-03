use std::collections::HashMap;

use crate::bytecode::{CompiledFunction, Instruction};
use crate::il_analysis::{IlAnalyzer, IlWarning};

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

    fn analyze_function(&mut self, function: &CompiledFunction) -> Vec<IlWarning> {
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
            if let Some((function_name, params)) = call_parts
                && let Some(&expected) = self.function_arities.get(function_name)
            {
                let got = params.len();
                if got != expected {
                    warnings.push(IlWarning::CallArityMismatch {
                        function_name: function_name.clone(),
                        expected,
                        got,
                        instruction_index: index,
                    });
                }
            }
        }

        warnings
    }
}
