use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Expression, Function, Module, Statement};
use crate::types::{FileId, Spanned};
use std::collections::HashMap;

pub struct InfiniteLoopAnalyzer {
    variable_assignments: HashMap<String, bool>,
}

impl InfiniteLoopAnalyzer {
    pub fn new() -> Self {
        Self {
            variable_assignments: HashMap::new(),
        }
    }

    fn analyze_function(&mut self, func: &Function, file_id: FileId) -> Vec<Warning> {
        self.variable_assignments.clear();
        let mut warnings = Vec::new();
        self.collect_variable_assignments(&func.body.statements);
        self.analyze_statements(&func.body.statements, file_id, &mut warnings);
        warnings
    }

    fn collect_variable_assignments(&mut self, statements: &[Statement]) {
        for statement in statements {
            match statement {
                Statement::Assignment {
                    variable,
                    expression,
                    ..
                } => {
                    if let Expression::BooleanLiteral { value: true, .. } = expression {
                        self.variable_assignments.insert(variable.clone(), true);
                    } else {
                        self.variable_assignments.insert(variable.clone(), false);
                    }
                }
                Statement::VariableAssignment { variable, .. } => {
                    self.variable_assignments.insert(variable.clone(), false);
                }
                Statement::If { body, .. } | Statement::While { body, .. } => {
                    self.collect_variable_assignments(body);
                }
                _ => {}
            }
        }
    }

    fn is_variable_modified_in_loop(&self, var_name: &str, statements: &[Statement]) -> bool {
        for statement in statements {
            match statement {
                Statement::VariableAssignment { variable, .. } => {
                    if variable == var_name {
                        return true;
                    }
                }
                Statement::If { body, .. } | Statement::While { body, .. } => {
                    if self.is_variable_modified_in_loop(var_name, body) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn analyze_statements(
        &self,
        statements: &[Statement],
        file_id: FileId,
        warnings: &mut Vec<Warning>,
    ) {
        for statement in statements {
            match statement {
                Statement::While {
                    condition, body, ..
                } => {
                    let is_infinite = if self.is_constant_true(condition) {
                        true
                    } else if let Expression::Variable { name, .. } = condition {
                        if let Some(&is_true) = self.variable_assignments.get(name) {
                            is_true && !self.is_variable_modified_in_loop(name, body)
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if is_infinite && !self.has_return_statement(body) {
                        warnings.push(Warning::PotentialInfiniteLoop {
                            span: condition.span(),
                            file_id,
                        });
                    }
                    self.analyze_statements(body, file_id, warnings);
                }
                Statement::If { body, .. } => {
                    self.analyze_statements(body, file_id, warnings);
                }
                _ => {}
            }
        }
    }

    fn is_constant_true(&self, expr: &Expression) -> bool {
        match expr {
            Expression::BooleanLiteral { value: true, .. } => true,
            Expression::Variable { .. } => false,
            _ => false,
        }
    }

    fn has_return_statement(&self, statements: &[Statement]) -> bool {
        for statement in statements {
            match statement {
                Statement::Return(_) => return true,
                Statement::If { body, .. } | Statement::While { body, .. } => {
                    if self.has_return_statement(body) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
}

impl Analyzer for InfiniteLoopAnalyzer {
    fn name(&self) -> &str {
        "infinite-loops"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                warnings.extend(self.analyze_function(func, file_id));
            }
        }

        warnings
    }
}

impl Default for InfiniteLoopAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
