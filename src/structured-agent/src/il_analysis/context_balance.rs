use crate::bytecode::{BytecodeRef, Instruction};
use crate::il_analysis::{IlAnalyzer, IlWarning};

pub struct ContextBalanceAnalyzer;

impl ContextBalanceAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContextBalanceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl IlAnalyzer for ContextBalanceAnalyzer {
    fn name(&self) -> &str {
        "context-balance"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let mut depth: i32 = 0;
        let mut warnings = Vec::new();

        for (index, instruction) in function.instructions.iter().enumerate() {
            match instruction {
                Instruction::CtxChild { .. } => {
                    depth += 1;
                }
                Instruction::CtxRestore => {
                    depth -= 1;
                    if depth < 0 {
                        warnings.push(IlWarning::ContextUnderflow {
                            instruction_index: index,
                        });
                        depth = 0;
                    }
                }
                _ => {}
            }
        }

        if depth > 0 {
            warnings.push(IlWarning::ContextNotRestored { depth });
        }

        warnings
    }
}
