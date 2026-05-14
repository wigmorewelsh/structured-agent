use crate::analysis::{Analyzer, Warning};
use structured_agent_ast::ast::{Function, Statement};
use structured_agent_ast::types::{FileId, Spanned};

pub struct OrphanedInjectionAnalyzer;

impl OrphanedInjectionAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn check_block(&self, stmts: &[Statement], file_id: FileId, warnings: &mut Vec<Warning>) {
        if let Some(last) = stmts.last() {
            if let Statement::Injection(expr) = last {
                warnings.push(Warning::OrphanedInjection {
                    span: expr.span(),
                    file_id,
                });
            }
        }
        for stmt in stmts {
            self.recurse_into_statement(stmt, file_id, warnings);
        }
    }

    fn recurse_into_statement(
        &self,
        stmt: &Statement,
        file_id: FileId,
        warnings: &mut Vec<Warning>,
    ) {
        match stmt {
            Statement::If {
                body, else_body, ..
            } => {
                self.check_block(body, file_id, warnings);
                if let Some(else_stmts) = else_body {
                    self.check_block(else_stmts, file_id, warnings);
                }
            }
            Statement::While { body, .. } => {
                self.check_block(body, file_id, warnings);
            }
            Statement::ForIn { body, .. } => {
                self.check_block(body, file_id, warnings);
            }
            _ => {}
        }
    }
}

impl Default for OrphanedInjectionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for OrphanedInjectionAnalyzer {
    fn name(&self) -> &str {
        "orphaned_injection"
    }

    fn analyze_function(&mut self, func: &Function, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();
        for stmt in &func.body.statements {
            self.recurse_into_statement(stmt, file_id, &mut warnings);
        }
        warnings
    }
}
