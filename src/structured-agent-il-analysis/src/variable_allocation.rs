use std::collections::HashSet;

use crate::{IlAnalyzer, IlWarning, instruction_reads, instruction_writes};
use structured_agent_il::BytecodeRef;

pub struct VariableAllocationAnalyzer;

impl VariableAllocationAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VariableAllocationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl IlAnalyzer for VariableAllocationAnalyzer {
    fn name(&self) -> &str {
        "variable-allocation"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let param_count = function.parameters.len();
        let mut initialized: HashSet<u32> = (1..=(param_count as u32)).collect();
        let mut warnings = Vec::new();

        for (index, instruction) in function.instructions.iter().enumerate() {
            for slot in instruction_reads(instruction) {
                if !initialized.contains(&slot.0) {
                    warnings.push(IlWarning::VariableUsedBeforeAllocation {
                        name: format!("s{}", slot.0),
                        instruction_index: index,
                        function_name: None,
                    });
                }
            }
            if let Some(dest) = instruction_writes(instruction) {
                initialized.insert(dest.0);
            }
        }

        warnings
    }
}
