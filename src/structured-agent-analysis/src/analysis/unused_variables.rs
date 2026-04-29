use crate::analysis::{Analyzer, Warning};
use std::collections::HashMap;
use structured_agent_ast::ast::{Expression, Function, Statement};
use structured_agent_ast::types::{FileId, Span};

#[derive(Debug, Clone)]
struct VariableInfo {
    declaration_span: Span,
    reads: Vec<Span>,
}

pub struct UnusedVariableAnalyzer {
    variables: HashMap<String, VariableInfo>,
}

impl UnusedVariableAnalyzer {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    fn track_declaration(&mut self, name: &str, span: Span) {
        self.variables.insert(
            name.to_string(),
            VariableInfo {
                declaration_span: span,
                reads: Vec::new(),
            },
        );
    }

    fn track_read(&mut self, name: &str, span: Span) {
        if let Some(info) = self.variables.get_mut(name) {
            info.reads.push(span);
        }
    }

    fn analyze_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Assignment {
                variable,
                expression,
                span,
            } => {
                self.track_declaration(variable, *span);
                self.analyze_expression(expression);
            }
            Statement::VariableAssignment {
                variable: _,
                expression,
                span: _,
            } => {
                self.analyze_expression(expression);
            }
            Statement::Injection(expr) => {
                self.analyze_expression(expr);
            }
            Statement::ExpressionStatement(expr) => {
                self.analyze_expression(expr);
            }
            Statement::If {
                condition, body, ..
            } => {
                self.analyze_expression(condition);
                for stmt in body {
                    self.analyze_statement(stmt);
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                self.analyze_expression(condition);
                for stmt in body {
                    self.analyze_statement(stmt);
                }
            }
            Statement::Return(expr) => {
                self.analyze_expression(expr);
            }
            Statement::Yield { .. } => {}
        }
    }

    fn analyze_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Variable { name, span } => {
                self.track_read(name, *span);
            }
            Expression::Call { arguments, .. } => {
                for arg in arguments {
                    self.analyze_expression(arg);
                }
            }
            Expression::Select(select_expr) => {
                for clause in &select_expr.clauses {
                    self.analyze_expression(&clause.expression_to_run);
                }
            }
            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.analyze_expression(condition);
                self.analyze_expression(then_expr);
                self.analyze_expression(else_expr);
            }
            Expression::StructLiteral { fields, .. } => {
                for (_, expr) in fields {
                    self.analyze_expression(expr);
                }
            }
            Expression::FieldAccess { base, .. } => {
                self.analyze_expression(base);
            }
            Expression::MethodCall { receiver, args, .. } => {
                self.analyze_expression(receiver);
                for arg in args {
                    self.analyze_expression(arg);
                }
            }
            Expression::StringLiteral { .. }
            | Expression::BooleanLiteral { .. }
            | Expression::IntLiteral { .. }
            | Expression::ListLiteral { .. }
            | Expression::UnitLiteral { .. }
            | Expression::Placeholder { .. } => {}
        }
    }

    fn check_unused_variables(&self, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for (name, info) in &self.variables {
            if info.reads.is_empty() {
                warnings.push(Warning::UnusedVariable {
                    name: name.clone(),
                    span: info.declaration_span,
                    file_id,
                });
            }
        }

        warnings
    }
}

impl Analyzer for UnusedVariableAnalyzer {
    fn name(&self) -> &str {
        "unused-variables"
    }

    fn analyze_function(&mut self, func: &Function, file_id: FileId) -> Vec<Warning> {
        self.variables.clear();

        for param in &func.parameters {
            self.track_declaration(&param.name, param.span);
        }

        for statement in &func.body.statements {
            self.analyze_statement(statement);
        }

        self.check_unused_variables(file_id)
    }
}

impl Default for UnusedVariableAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
