use std::collections::HashSet;

use crate::bytecode::{BytecodeRef, Instruction};
use crate::il_analysis::{IlAnalyzer, IlWarning};

pub struct DoubleDropAnalyzer;

impl DoubleDropAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DoubleDropAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl IlAnalyzer for DoubleDropAnalyzer {
    fn name(&self) -> &str {
        "double-drop"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let mut dropped: HashSet<String> = HashSet::new();
        let mut warnings = Vec::new();

        for (index, instruction) in function.instructions.iter().enumerate() {
            if let Instruction::Drop { name } = instruction
                && !dropped.insert(name.clone())
            {
                warnings.push(IlWarning::DoubleDrop {
                    name: name.clone(),
                    instruction_index: index,
                });
            }
        }

        warnings
    }
}
