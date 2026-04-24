use crate::analysis::{Analyzer, Warning};
use structured_agent_ast::ast::{Definition, Function, Module, Statement};
use structured_agent_ast::types::FileId;

pub struct MissingReturnOrInjectionAnalyzer;

impl MissingReturnOrInjectionAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn has_return_or_injection(statements: &[Statement]) -> bool {
        for stmt in statements {
            match stmt {
                Statement::Return(_) | Statement::Injection(_) => return true,
                Statement::If { body, .. } | Statement::While { body, .. } => {
                    if Self::has_return_or_injection(body) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn check_function(func: &Function, file_id: FileId, warnings: &mut Vec<Warning>) {
        if !Self::has_return_or_injection(&func.body.statements) {
            warnings.push(Warning::MissingReturnOrInjection {
                name: func.name.clone(),
                span: func.span,
                file_id,
            });
        }
    }
}

impl Default for MissingReturnOrInjectionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for MissingReturnOrInjectionAnalyzer {
    fn name(&self) -> &str {
        "missing_return_or_injection"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    Self::check_function(func, file_id, &mut warnings);
                }
                Definition::TraitImpl(trait_impl) => {
                    for func in &trait_impl.functions {
                        Self::check_function(func, file_id, &mut warnings);
                    }
                }
                _ => {}
            }
        }

        warnings
    }
}
