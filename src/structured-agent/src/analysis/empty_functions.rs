use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Module};
use crate::types::FileId;

pub struct EmptyFunctionAnalyzer;

impl EmptyFunctionAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EmptyFunctionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for EmptyFunctionAnalyzer {
    fn name(&self) -> &str {
        "empty_functions"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                if func.body.statements.is_empty() {
                    warnings.push(Warning::EmptyFunction {
                        name: func.name.clone(),
                        span: func.span,
                        file_id,
                    });
                }
            }
        }

        warnings
    }
}
