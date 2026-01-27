use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Module, Statement};
use crate::types::{FileId, Span};
use std::collections::HashMap;

pub struct VariableShadowingAnalyzer;

impl VariableShadowingAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn analyze_statements(
        &self,
        statements: &[Statement],
        file_id: FileId,
        scopes: &mut Vec<HashMap<String, Span>>,
        warnings: &mut Vec<Warning>,
    ) {
        scopes.push(HashMap::new());

        for stmt in statements {
            match stmt {
                Statement::Assignment { variable, span, .. } => {
                    for outer_scope in scopes.iter().rev().skip(1) {
                        if let Some(&outer_span) = outer_scope.get(variable) {
                            warnings.push(Warning::VariableShadowing {
                                name: variable.clone(),
                                inner_span: *span,
                                outer_span,
                                file_id,
                            });
                            break;
                        }
                    }

                    if let Some(current_scope) = scopes.last_mut() {
                        current_scope.insert(variable.clone(), *span);
                    }
                }
                Statement::If { body, .. } => {
                    self.analyze_statements(body, file_id, scopes, warnings);
                }
                Statement::While { body, .. } => {
                    self.analyze_statements(body, file_id, scopes, warnings);
                }
                _ => {}
            }
        }

        scopes.pop();
    }
}

impl Default for VariableShadowingAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for VariableShadowingAnalyzer {
    fn name(&self) -> &str {
        "variable_shadowing"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                let mut scopes: Vec<HashMap<String, Span>> = Vec::new();
                scopes.push(HashMap::new());

                for param in &func.parameters {
                    if let Some(current_scope) = scopes.last_mut() {
                        current_scope.insert(param.name.clone(), param.span);
                    }
                }

                scopes.push(HashMap::new());

                for statement in &func.body.statements {
                    match statement {
                        Statement::Assignment { variable, span, .. } => {
                            for outer_scope in scopes.iter().rev().skip(1) {
                                if let Some(&outer_span) = outer_scope.get(variable) {
                                    warnings.push(Warning::VariableShadowing {
                                        name: variable.clone(),
                                        inner_span: *span,
                                        outer_span,
                                        file_id,
                                    });
                                    break;
                                }
                            }

                            if let Some(current_scope) = scopes.last_mut() {
                                current_scope.insert(variable.clone(), *span);
                            }
                        }
                        Statement::If { body, .. } => {
                            self.analyze_statements(body, file_id, &mut scopes, &mut warnings);
                        }
                        Statement::While { body, .. } => {
                            self.analyze_statements(body, file_id, &mut scopes, &mut warnings);
                        }
                        _ => {}
                    }
                }
            }
        }

        warnings
    }
}
