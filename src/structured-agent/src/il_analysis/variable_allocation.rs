use std::collections::HashSet;

use crate::bytecode::CompiledFunction;
use crate::il_analysis::{IlAnalyzer, IlWarning, instruction_reads, instruction_writes};

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

    fn analyze_function(&mut self, function: &CompiledFunction) -> Vec<IlWarning> {
        let mut allocated: HashSet<String> =
            function.parameters.iter().map(|p| p.name.clone()).collect();

        let mut warnings = Vec::new();

        for (index, instruction) in function.instructions.iter().enumerate() {
            for var in instruction_reads(instruction) {
                if !allocated.contains(var) {
                    warnings.push(IlWarning::VariableUsedBeforeAllocation {
                        name: var.to_string(),
                        instruction_index: index,
                    });
                }
            }

            if let Some(dest) = instruction_writes(instruction) {
                allocated.insert(dest.to_string());
            }
        }

        warnings
    }
}
