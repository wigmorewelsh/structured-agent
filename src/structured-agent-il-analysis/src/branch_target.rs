use structured_agent_il::{BytecodeRef, Instruction};

use crate::{IlAnalyzer, IlWarning};

pub struct BranchTargetAnalyzer;

impl BranchTargetAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BranchTargetAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl IlAnalyzer for BranchTargetAnalyzer {
    fn name(&self) -> &str {
        "branch-target"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let len = function.instructions.len();
        let mut warnings = Vec::new();

        for (index, instruction) in function.instructions.iter().enumerate() {
            let targets: Vec<i32> = match instruction {
                Instruction::Br { offset } => vec![*offset],
                Instruction::BrFalse { offset, .. } => vec![*offset],
                Instruction::BrTrue { offset, .. } => vec![*offset],
                Instruction::Switch { offsets, .. } => offsets.clone(),
                _ => vec![],
            };

            for target in targets {
                if target < 0 || target as usize >= len {
                    warnings.push(IlWarning::InvalidBranchTarget {
                        instruction_index: index,
                        target,
                    });
                }
            }
        }

        warnings
    }
}
