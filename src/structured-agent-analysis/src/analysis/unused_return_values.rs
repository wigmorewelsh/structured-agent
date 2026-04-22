use crate::analysis::{Analyzer, Warning};
use structured_agent_ast::ast::{Definition, Expression, Module, Statement, Type};
use structured_agent_ast::types::FileId;
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
                    let returns_value = func.return_type != Type::simple("Unit");
                    self.function_return_types
                        .insert(func.name.clone(), returns_value);
                }
                Definition::ExternalFunction(ext_func) => {
                    let returns_value = ext_func.return_type != Type::simple("Unit");
                    self.function_return_types
                        .insert(ext_func.name.clone(), returns_value);
                }
                Definition::Struct(_)
                | Definition::Use(_)
                | Definition::ModuleHeader { .. }
                | Definition::Signature(_)
                | Definition::Trait(_)
                | Definition::TraitImpl(_)
                | Definition::InlineModule { .. } => {}
            }
        }
    }

    fn analyze_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::ExpressionStatement(expr) => {
                if let Expression::Call { function, span, .. } = expr
                    && let Some(&returns_value) = self.function_return_types.get(function)
                    && returns_value
                {
                    self.warnings.push(Warning::UnusedReturnValue {
                        function_name: function.clone(),
                        span: *span,
                        file_id: self.file_id,
                    });
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
            Expression::FieldAccess { .. } => {}
            Expression::Variable { .. }
            | Expression::StringLiteral { .. }
            | Expression::BooleanLiteral { .. }
            | Expression::IntLiteral { .. }
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
