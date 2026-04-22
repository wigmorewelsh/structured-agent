use std::collections::HashSet;

use crate::{IlAnalyzer, IlWarning};
use structured_agent_il::{BytecodeRef, Instruction};

pub struct VariableDropAnalyzer;

impl VariableDropAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VariableDropAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl IlAnalyzer for VariableDropAnalyzer {
    fn name(&self) -> &str {
        "variable-drop"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let mut declared: HashSet<String> = HashSet::new();
        let mut dropped: HashSet<String> = HashSet::new();
        let mut returned: HashSet<String> = HashSet::new();

        for instruction in &function.instructions {
            match instruction {
                Instruction::Decl { name } => {
                    declared.insert(name.clone());
                }
                Instruction::Drop { name } => {
                    dropped.insert(name.clone());
                }
                Instruction::Ret { var } => {
                    returned.insert(var.clone());
                }
                _ => {}
            }
        }

        declared
            .into_iter()
            .filter(|name| !dropped.contains(name) && !returned.contains(name))
            .map(|name| IlWarning::VariableNotDropped { name })
            .collect()
    }
}
