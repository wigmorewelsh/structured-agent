use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Expression, Module, Statement};
use crate::types::{FileId, Spanned};
use std::collections::HashMap;

pub struct ConstantConditionAnalyzer;

impl ConstantConditionAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn is_constant_condition(
        &self,
        expr: &Expression,
        variable_values: &HashMap<String, bool>,
    ) -> Option<bool> {
        match expr {
            Expression::BooleanLiteral { value, .. } => Some(*value),
            Expression::Variable { name, .. } => variable_values.get(name).copied(),
            _ => None,
        }
    }

    fn collect_assignments(statements: &[Statement], values: &mut HashMap<String, bool>) {
        for stmt in statements {
            match stmt {
                Statement::Assignment {
                    variable,
                    expression,
                    ..
                } => {
                    if let Expression::BooleanLiteral { value, .. } = expression {
                        values.insert(variable.clone(), *value);
                    } else {
                        values.remove(variable);
                    }
                }
                Statement::VariableAssignment { variable, .. } => {
                    values.remove(variable);
                }
                Statement::If { body, .. } => {
                    Self::collect_assignments(body, values);
                }
                Statement::While { body, .. } => {
                    Self::collect_assignments(body, values);
                }
                _ => {}
            }
        }
    }

    fn analyze_expression(
        &self,
        expr: &Expression,
        file_id: FileId,
        variable_values: &HashMap<String, bool>,
        warnings: &mut Vec<Warning>,
    ) {
        match expr {
            Expression::Call { arguments, .. } => {
                for arg in arguments {
                    self.analyze_expression(arg, file_id, variable_values, warnings);
                }
            }
            Expression::Select(select_expr) => {
                for clause in &select_expr.clauses {
                    self.analyze_expression(
                        &clause.expression_to_run,
                        file_id,
                        variable_values,
                        warnings,
                    );
                    self.analyze_expression(
                        &clause.expression_next,
                        file_id,
                        variable_values,
                        warnings,
                    );
                }
            }
            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                if let Some(value) = self.is_constant_condition(condition, variable_values) {
                    warnings.push(Warning::ConstantCondition {
                        condition_value: value,
                        span: condition.span(),
                        file_id,
                    });
                }
                self.analyze_expression(then_expr, file_id, variable_values, warnings);
                self.analyze_expression(else_expr, file_id, variable_values, warnings);
            }
            _ => {}
        }
    }

    fn analyze_statement(
        &self,
        stmt: &Statement,
        file_id: FileId,
        variable_values: &HashMap<String, bool>,
        warnings: &mut Vec<Warning>,
    ) {
        match stmt {
            Statement::If {
                condition, body, ..
            } => {
                if let Some(value) = self.is_constant_condition(condition, variable_values) {
                    warnings.push(Warning::ConstantCondition {
                        condition_value: value,
                        span: condition.span(),
                        file_id,
                    });
                }

                for stmt in body {
                    self.analyze_statement(stmt, file_id, variable_values, warnings);
                }
            }
            Statement::While { body, .. } => {
                for stmt in body {
                    self.analyze_statement(stmt, file_id, variable_values, warnings);
                }
            }
            Statement::Assignment { expression, .. } => {
                self.analyze_expression(expression, file_id, variable_values, warnings);
            }
            Statement::VariableAssignment { expression, .. } => {
                self.analyze_expression(expression, file_id, variable_values, warnings);
            }
            Statement::Injection(expr) => {
                self.analyze_expression(expr, file_id, variable_values, warnings);
            }
            Statement::ExpressionStatement(expr) => {
                self.analyze_expression(expr, file_id, variable_values, warnings);
            }
            Statement::Return(expr) => {
                self.analyze_expression(expr, file_id, variable_values, warnings);
            }
        }
    }
}

impl Default for ConstantConditionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for ConstantConditionAnalyzer {
    fn name(&self) -> &str {
        "constant_conditions"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                let mut variable_values = HashMap::new();
                Self::collect_assignments(&func.body.statements, &mut variable_values);

                for statement in &func.body.statements {
                    self.analyze_statement(statement, file_id, &variable_values, &mut warnings);
                }
            }
        }

        warnings
    }
}
