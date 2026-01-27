use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Expression, Module, Statement};
use crate::types::{FileId, Span};
use std::collections::{HashMap, HashSet};

pub struct OverwrittenValueAnalyzer;

impl OverwrittenValueAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn collect_reads_in_expression(expr: &Expression, reads: &mut HashSet<String>) {
        match expr {
            Expression::Variable { name, .. } => {
                reads.insert(name.clone());
            }
            Expression::Call { arguments, .. } => {
                for arg in arguments {
                    Self::collect_reads_in_expression(arg, reads);
                }
            }
            Expression::Select(select_expr) => {
                for clause in &select_expr.clauses {
                    Self::collect_reads_in_expression(&clause.expression_to_run, reads);
                    Self::collect_reads_in_expression(&clause.expression_next, reads);
                }
            }
            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                Self::collect_reads_in_expression(condition, reads);
                Self::collect_reads_in_expression(then_expr, reads);
                Self::collect_reads_in_expression(else_expr, reads);
            }
            _ => {}
        }
    }

    fn collect_reads_in_statement(stmt: &Statement, reads: &mut HashSet<String>) {
        match stmt {
            Statement::Injection(expr) => {
                Self::collect_reads_in_expression(expr, reads);
            }
            Statement::Assignment { expression, .. } => {
                Self::collect_reads_in_expression(expression, reads);
            }
            Statement::VariableAssignment {
                variable,
                expression,
                ..
            } => {
                reads.insert(variable.clone());
                Self::collect_reads_in_expression(expression, reads);
            }
            Statement::ExpressionStatement(expr) => {
                Self::collect_reads_in_expression(expr, reads);
            }
            Statement::If {
                condition, body, ..
            } => {
                Self::collect_reads_in_expression(condition, reads);
                for stmt in body {
                    Self::collect_reads_in_statement(stmt, reads);
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                Self::collect_reads_in_expression(condition, reads);
                for stmt in body {
                    Self::collect_reads_in_statement(stmt, reads);
                }
            }
            Statement::Return(expr) => {
                Self::collect_reads_in_expression(expr, reads);
            }
        }
    }

    fn analyze_statements(
        statements: &[Statement],
        file_id: FileId,
        assignments: &mut HashMap<String, Span>,
        reads: &mut HashSet<String>,
        warnings: &mut Vec<Warning>,
    ) {
        for stmt in statements {
            match stmt {
                Statement::Assignment {
                    variable,
                    expression,
                    span,
                } => {
                    if let Some(&old_span) = assignments.get(variable) {
                        if !reads.contains(variable) {
                            warnings.push(Warning::OverwrittenValue {
                                name: variable.clone(),
                                span: old_span,
                                file_id,
                            });
                        }
                    }
                    reads.remove(variable);
                    assignments.insert(variable.clone(), *span);
                    Self::collect_reads_in_expression(expression, reads);
                }
                Statement::VariableAssignment { variable, .. } => {
                    reads.insert(variable.clone());
                }
                Statement::Injection(expr) => {
                    Self::collect_reads_in_expression(expr, reads);
                }
                Statement::ExpressionStatement(expr) => {
                    Self::collect_reads_in_expression(expr, reads);
                }
                Statement::Return(expr) => {
                    Self::collect_reads_in_expression(expr, reads);
                }
                Statement::If {
                    condition, body, ..
                } => {
                    Self::collect_reads_in_expression(condition, reads);
                    for stmt in body {
                        Self::collect_reads_in_statement(stmt, reads);
                    }
                }
                Statement::While {
                    condition, body, ..
                } => {
                    Self::collect_reads_in_expression(condition, reads);
                    for stmt in body {
                        Self::collect_reads_in_statement(stmt, reads);
                    }
                }
            }
        }
    }
}

impl Default for OverwrittenValueAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for OverwrittenValueAnalyzer {
    fn name(&self) -> &str {
        "overwritten_values"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                let mut assignments: HashMap<String, Span> = HashMap::new();
                let mut reads: HashSet<String> = HashSet::new();

                Self::analyze_statements(
                    &func.body.statements,
                    file_id,
                    &mut assignments,
                    &mut reads,
                    &mut warnings,
                );
            }
        }

        warnings
    }
}
