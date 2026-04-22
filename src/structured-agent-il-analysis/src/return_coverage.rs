use structured_agent_il::{BytecodeRef, Instruction};

use crate::{IlAnalyzer, IlWarning};

pub struct ReturnCoverageAnalyzer;

impl ReturnCoverageAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReturnCoverageAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl IlAnalyzer for ReturnCoverageAnalyzer {
    fn name(&self) -> &str {
        "return-coverage"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        if function.instructions.is_empty() {
            return vec![];
        }

        let has_ret = function
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::Ret { .. }));

        if has_ret {
            vec![]
        } else {
            vec![IlWarning::NoReturnPath]
        }
    }
}
