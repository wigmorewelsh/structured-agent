use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Module, Statement};
use crate::types::FileId;

pub struct EmptyBlockAnalyzer;

impl EmptyBlockAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn analyze_statement(stmt: &Statement, file_id: FileId, warnings: &mut Vec<Warning>) {
        match stmt {
            Statement::If {
                condition: _,
                body,
                else_body,
                span,
            } => {
                if body.is_empty() {
                    warnings.push(Warning::EmptyBlock {
                        block_type: "if".to_string(),
                        span: *span,
                        file_id,
                    });
                }
                for stmt in body {
                    Self::analyze_statement(stmt, file_id, warnings);
                }

                if let Some(else_stmts) = else_body {
                    if else_stmts.is_empty() {
                        warnings.push(Warning::EmptyBlock {
                            block_type: "else".to_string(),
                            span: *span,
                            file_id,
                        });
                    }
                    for stmt in else_stmts {
                        Self::analyze_statement(stmt, file_id, warnings);
                    }
                }
            }
            Statement::While {
                condition: _,
                body,
                span,
            } => {
                if body.is_empty() {
                    warnings.push(Warning::EmptyBlock {
                        block_type: "while".to_string(),
                        span: *span,
                        file_id,
                    });
                }
                for stmt in body {
                    Self::analyze_statement(stmt, file_id, warnings);
                }
            }
            _ => {}
        }
    }
}

impl Default for EmptyBlockAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for EmptyBlockAnalyzer {
    fn name(&self) -> &str {
        "empty_blocks"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                for statement in &func.body.statements {
                    Self::analyze_statement(statement, file_id, &mut warnings);
                }
            }
        }

        warnings
    }
}
