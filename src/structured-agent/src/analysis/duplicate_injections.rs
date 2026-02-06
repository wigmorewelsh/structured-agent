use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Expression, Module, Statement};
use crate::types::{FileId, Spanned};

pub struct DuplicateInjectionAnalyzer;

#[derive(Debug, Clone, PartialEq)]
enum InjectionValue {
    StringLiteral(String),
    Variable(String),
}

impl DuplicateInjectionAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn extract_injection_value(expr: &Expression) -> Option<InjectionValue> {
        match expr {
            Expression::StringLiteral { value, .. } => {
                Some(InjectionValue::StringLiteral(value.clone()))
            }
            Expression::Variable { name, .. } => Some(InjectionValue::Variable(name.clone())),
            _ => None,
        }
    }

    fn analyze_statements(statements: &[Statement], file_id: FileId, warnings: &mut Vec<Warning>) {
        let mut last_injection: Option<InjectionValue> = None;

        for stmt in statements {
            match stmt {
                Statement::Injection(value) => {
                    if let Some(current_value) = Self::extract_injection_value(value) {
                        if let Some(last_value) = &last_injection
                            && *last_value == current_value
                        {
                            warnings.push(Warning::DuplicateInjection {
                                span: value.span(),
                                file_id,
                            });
                        }
                        last_injection = Some(current_value);
                    } else {
                        last_injection = None;
                    }
                }
                Statement::If { body, .. } | Statement::While { body, .. } => {
                    last_injection = None;
                    Self::analyze_statements(body, file_id, warnings);
                }
                _ => {}
            }
        }
    }
}

impl Default for DuplicateInjectionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for DuplicateInjectionAnalyzer {
    fn name(&self) -> &str {
        "duplicate_injections"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                Self::analyze_statements(&func.body.statements, file_id, &mut warnings);
            }
        }

        warnings
    }
}
