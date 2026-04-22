use std::collections::HashSet;

use crate::{IlAnalyzer, IlWarning};
use structured_agent_il::{BytecodeRef, Instruction};

pub struct DuplicateDeclAnalyzer;

impl DuplicateDeclAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DuplicateDeclAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl IlAnalyzer for DuplicateDeclAnalyzer {
    fn name(&self) -> &str {
        "duplicate-decl"
    }

    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let mut scope_stack: Vec<HashSet<String>> = vec![HashSet::new()];
        let mut warnings = Vec::new();

        for (index, instruction) in function.instructions.iter().enumerate() {
            match instruction {
                Instruction::CtxChild { .. } => {
                    scope_stack.push(HashSet::new());
                }
                Instruction::CtxRestore => {
                    if scope_stack.len() > 1 {
                        scope_stack.pop();
                    }
                }
                Instruction::Decl { name } => {
                    let current_scope = scope_stack.last_mut().unwrap();
                    if !current_scope.insert(name.clone()) {
                        warnings.push(IlWarning::DuplicateDeclaration {
                            name: name.clone(),
                            instruction_index: index,
                        });
                    }
                }
                _ => {}
            }
        }

        warnings
    }
}
