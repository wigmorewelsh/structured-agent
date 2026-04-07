use std::collections::HashSet;

use crate::bytecode::{BytecodeRef, Instruction};
use crate::il_analysis::{IlAnalyzer, IlWarning};

pub struct UnreachableInstructionAnalyzer;

impl UnreachableInstructionAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnreachableInstructionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl IlAnalyzer for UnreachableInstructionAnalyzer {
    fn name(&self) -> &str {
        "unreachable-instructions"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let mut targets: HashSet<usize> = HashSet::new();
        for instruction in &function.instructions {
            match instruction {
                Instruction::Br { offset } => {
                    if *offset >= 0 {
                        targets.insert(*offset as usize);
                    }
                }
                Instruction::BrFalse { offset, .. } | Instruction::BrTrue { offset, .. } => {
                    if *offset >= 0 {
                        targets.insert(*offset as usize);
                    }
                }
                Instruction::Switch { offsets, .. } => {
                    for &offset in offsets {
                        if offset >= 0 {
                            targets.insert(offset as usize);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut warnings = Vec::new();
        let mut reachable = true;

        for (index, instruction) in function.instructions.iter().enumerate() {
            if targets.contains(&index) {
                reachable = true;
            }

            if !reachable {
                warnings.push(IlWarning::UnreachableInstruction {
                    instruction_index: index,
                });
            }

            match instruction {
                Instruction::Ret { .. } | Instruction::Br { .. } => {
                    reachable = false;
                }
                _ => {}
            }
        }

        warnings
    }
}
