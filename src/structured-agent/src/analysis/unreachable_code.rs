use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Expression, Function, Module, Statement};
use crate::types::{FileId, Span, Spanned};
use std::collections::HashSet;

pub struct ReachabilityAnalyzer {
    reachable: HashSet<Span>,
    all_statements: Vec<Span>,
}

impl ReachabilityAnalyzer {
    pub fn new() -> Self {
        Self {
            reachable: HashSet::new(),
            all_statements: Vec::new(),
        }
    }

    fn analyze_function(&mut self, func: &Function, file_id: FileId) -> Vec<Warning> {
        self.reachable.clear();
        self.all_statements.clear();

        self.collect_all_statements(&func.body.statements);
        self.analyze_statements(&func.body.statements, true);

        self.check_unreachable(file_id)
    }

    fn collect_all_statements(&mut self, statements: &[Statement]) {
        for statement in statements {
            let span = match statement {
                Statement::Injection(expr) => expr.span(),
                Statement::Assignment { span, .. } => *span,
                Statement::VariableAssignment { span, .. } => *span,
                Statement::ExpressionStatement(expr) => expr.span(),
                Statement::If { span, body, .. } => {
                    self.collect_all_statements(body);
                    *span
                }
                Statement::While { span, body, .. } => {
                    self.collect_all_statements(body);
                    *span
                }
                Statement::Return(expr) => expr.span(),
            };
            self.all_statements.push(span);
        }
    }

    fn analyze_statements(&mut self, statements: &[Statement], reachable: bool) -> bool {
        let mut current_reachable = reachable;

        for statement in statements {
            if current_reachable {
                let span = match statement {
                    Statement::Injection(expr) => expr.span(),
                    Statement::Assignment { span, .. } => *span,
                    Statement::VariableAssignment { span, .. } => *span,
                    Statement::ExpressionStatement(expr) => expr.span(),
                    Statement::If { span, .. } => *span,
                    Statement::While { span, .. } => *span,
                    Statement::Return(expr) => expr.span(),
                };
                self.reachable.insert(span);
            }

            match statement {
                Statement::If {
                    condition, body, ..
                } => {
                    if current_reachable {
                        if self.is_constant_true(condition) {
                            self.analyze_statements(body, true);
                        } else {
                            self.analyze_statements(body, current_reachable);
                        }
                    }
                }
                Statement::While {
                    condition, body, ..
                } => {
                    if current_reachable {
                        self.analyze_statements(body, true);
                        if self.is_constant_true(condition) {
                            current_reachable = false;
                        }
                    }
                }
                Statement::Return(_) => {
                    current_reachable = false;
                }
                _ => {}
            }
        }

        current_reachable
    }

    fn is_constant_true(&self, expr: &Expression) -> bool {
        matches!(expr, Expression::BooleanLiteral { value: true, .. })
    }

    fn check_unreachable(&self, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for span in &self.all_statements {
            if !self.reachable.contains(span) {
                warnings.push(Warning::UnreachableCode {
                    span: *span,
                    file_id,
                });
            }
        }

        warnings
    }
}

impl Analyzer for ReachabilityAnalyzer {
    fn name(&self) -> &str {
        "unreachable-code"
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

impl Default for ReachabilityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
