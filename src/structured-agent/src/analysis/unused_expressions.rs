use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Expression, Module, Statement};
use crate::types::FileId;

pub struct UnusedExpressionAnalyzer {
    warnings: Vec<Warning>,
    file_id: FileId,
}

impl UnusedExpressionAnalyzer {
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
            file_id: FileId::default(),
        }
    }

    fn analyze_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::ExpressionStatement(expr) => match expr {
                Expression::StringLiteral { span, .. }
                | Expression::BooleanLiteral { span, .. }
                | Expression::ListLiteral { span, .. }
                | Expression::UnitLiteral { span } => {
                    self.warnings.push(Warning::UnusedExpression {
                        span: *span,
                        file_id: self.file_id,
                    });
                }
                Expression::Variable { .. }
                | Expression::Call { .. }
                | Expression::Select(_)
                | Expression::IfElse { .. }
                | Expression::Placeholder { .. } => {
                    self.analyze_expression(expr);
                }
            },
            Statement::Injection(expr) => {
                self.analyze_expression(expr);
            }
            Statement::Assignment { expression, .. } => {
                self.analyze_expression(expression);
            }
            Statement::VariableAssignment { expression, .. } => {
                self.analyze_expression(expression);
            }
            Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                self.analyze_expression(condition);
                for stmt in body {
                    self.analyze_statement(stmt);
                }
                if let Some(else_stmts) = else_body {
                    for stmt in else_stmts {
                        self.analyze_statement(stmt);
                    }
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
        }
    }

    fn analyze_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Call { arguments, .. } => {
                for arg in arguments {
                    self.analyze_expression(arg);
                }
            }
            Expression::Select(select_expr) => {
                for clause in &select_expr.clauses {
                    self.analyze_expression(&clause.expression_to_run);
                    self.analyze_expression(&clause.expression_next);
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
            Expression::Variable { .. }
            | Expression::StringLiteral { .. }
            | Expression::BooleanLiteral { .. }
            | Expression::ListLiteral { .. }
            | Expression::UnitLiteral { .. }
            | Expression::Placeholder { .. } => {}
        }
    }

    fn analyze_function_body(&mut self, statements: &[Statement]) {
        for statement in statements {
            self.analyze_statement(statement);
        }
    }
}

impl Default for UnusedExpressionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for UnusedExpressionAnalyzer {
    fn name(&self) -> &str {
        "unused_expressions"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        self.warnings.clear();
        self.file_id = file_id;

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                self.analyze_function_body(&func.body.statements);
            }
        }

        self.warnings.clone()
    }
}
