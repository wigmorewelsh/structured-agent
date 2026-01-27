use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Expression, Module, Statement};
use crate::types::FileId;

pub struct RedundantSelectAnalyzer;

impl RedundantSelectAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn analyze_expression(&self, expr: &Expression, file_id: FileId, warnings: &mut Vec<Warning>) {
        match expr {
            Expression::Select(select_expr) => {
                if select_expr.clauses.len() == 1 {
                    warnings.push(Warning::RedundantSelect {
                        span: select_expr.span,
                        file_id,
                    });
                }
                for clause in &select_expr.clauses {
                    self.analyze_expression(&clause.expression_to_run, file_id, warnings);
                    self.analyze_expression(&clause.expression_next, file_id, warnings);
                }
            }
            Expression::Call { arguments, .. } => {
                for arg in arguments {
                    self.analyze_expression(arg, file_id, warnings);
                }
            }
            _ => {}
        }
    }

    fn analyze_statement(&self, stmt: &Statement, file_id: FileId, warnings: &mut Vec<Warning>) {
        match stmt {
            Statement::Injection(expr) => {
                self.analyze_expression(expr, file_id, warnings);
            }
            Statement::Assignment { expression, .. } => {
                self.analyze_expression(expression, file_id, warnings);
            }
            Statement::VariableAssignment { expression, .. } => {
                self.analyze_expression(expression, file_id, warnings);
            }
            Statement::ExpressionStatement(expr) => {
                self.analyze_expression(expr, file_id, warnings);
            }
            Statement::If {
                condition, body, ..
            } => {
                self.analyze_expression(condition, file_id, warnings);
                for stmt in body {
                    self.analyze_statement(stmt, file_id, warnings);
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                self.analyze_expression(condition, file_id, warnings);
                for stmt in body {
                    self.analyze_statement(stmt, file_id, warnings);
                }
            }
            Statement::Return(expr) => {
                self.analyze_expression(expr, file_id, warnings);
            }
        }
    }
}

impl Default for RedundantSelectAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for RedundantSelectAnalyzer {
    fn name(&self) -> &str {
        "redundant_select"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                for statement in &func.body.statements {
                    self.analyze_statement(statement, file_id, &mut warnings);
                }
            }
        }

        warnings
    }
}
