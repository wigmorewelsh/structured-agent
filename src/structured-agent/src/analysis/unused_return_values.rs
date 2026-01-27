use crate::analysis::{Analyzer, Warning};
use crate::ast::{Definition, Expression, Module, Statement};
use crate::types::FileId;
use std::collections::HashMap;

pub struct UnusedReturnValueAnalyzer {
    warnings: Vec<Warning>,
    file_id: FileId,
    function_return_types: HashMap<String, bool>,
}

impl UnusedReturnValueAnalyzer {
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
            file_id: FileId::default(),
            function_return_types: HashMap::new(),
        }
    }

    fn collect_function_signatures(&mut self, module: &Module) {
        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    let returns_value = !matches!(func.return_type, crate::ast::Type::Unit);
                    self.function_return_types
                        .insert(func.name.clone(), returns_value);
                }
                Definition::ExternalFunction(ext_func) => {
                    let returns_value = !matches!(ext_func.return_type, crate::ast::Type::Unit);
                    self.function_return_types
                        .insert(ext_func.name.clone(), returns_value);
                }
            }
        }
    }

    fn analyze_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::ExpressionStatement(expr) => {
                if let Expression::Call { function, span, .. } = expr {
                    if let Some(&returns_value) = self.function_return_types.get(function) {
                        if returns_value {
                            self.warnings.push(Warning::UnusedReturnValue {
                                function_name: function.clone(),
                                span: *span,
                                file_id: self.file_id,
                            });
                        }
                    }
                }
                self.analyze_expression(expr);
            }
            Statement::Injection(value) => {
                self.analyze_expression(value);
            }
            Statement::Assignment { expression, .. } => {
                self.analyze_expression(expression);
            }
            Statement::VariableAssignment { expression, .. } => {
                self.analyze_expression(expression);
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
            Expression::Variable { .. }
            | Expression::StringLiteral { .. }
            | Expression::BooleanLiteral { .. }
            | Expression::Placeholder { .. } => {}
        }
    }

    fn analyze_function_body(&mut self, statements: &[Statement]) {
        for statement in statements {
            self.analyze_statement(statement);
        }
    }
}

impl Default for UnusedReturnValueAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for UnusedReturnValueAnalyzer {
    fn name(&self) -> &str {
        "unused_return_values"
    }

    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        self.warnings.clear();
        self.file_id = file_id;
        self.function_return_types.clear();

        self.collect_function_signatures(module);

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                self.analyze_function_body(&func.body.statements);
            }
        }

        self.warnings.clone()
    }
}
