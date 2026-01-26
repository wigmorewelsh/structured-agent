use crate::ast::{Definition, Expression, Function, Module, Parameter, Statement, Type as AstType};
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span, Spanned};
use std::collections::HashMap;

#[derive(Debug)]
pub struct TypeChecker {
    function_signatures: HashMap<String, FunctionSignature>,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    parameters: Vec<Parameter>,
    return_type: AstType,
}

#[derive(Debug, Clone)]
struct TypeEnvironment {
    variables: HashMap<String, AstType>,
    parent: Option<Box<TypeEnvironment>>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            function_signatures: HashMap::new(),
        }
    }

    pub fn check_module(&mut self, module: &Module, file_id: FileId) -> Result<(), TypeError> {
        self.collect_function_signatures(module, file_id)?;
        self.check_all_functions(module, file_id)?;
        Ok(())
    }

    fn collect_function_signatures(
        &mut self,
        module: &Module,
        file_id: FileId,
    ) -> Result<(), TypeError> {
        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    self.validate_type(&func.return_type, func.span, file_id)?;
                    for param in &func.parameters {
                        self.validate_type(&param.param_type, param.span, file_id)?;
                    }

                    let signature = FunctionSignature {
                        parameters: func.parameters.clone(),
                        return_type: func.return_type.clone(),
                    };
                    self.function_signatures
                        .insert(func.name.clone(), signature);
                }
                Definition::ExternalFunction(ext_func) => {
                    self.validate_type(&ext_func.return_type, ext_func.span, file_id)?;
                    for param in &ext_func.parameters {
                        self.validate_type(&param.param_type, param.span, file_id)?;
                    }

                    let signature = FunctionSignature {
                        parameters: ext_func.parameters.clone(),
                        return_type: ext_func.return_type.clone(),
                    };
                    self.function_signatures
                        .insert(ext_func.name.clone(), signature);
                }
            }
        }
        Ok(())
    }

    fn validate_type(
        &self,
        ast_type: &AstType,
        span: Span,
        file_id: FileId,
    ) -> Result<(), TypeError> {
        match ast_type {
            AstType::Named(name) => match name.as_str() {
                "String" | "Boolean" | "Context" => Ok(()),
                _ => Err(TypeError::UnsupportedType {
                    type_name: name.clone(),
                    span,
                    file_id,
                }),
            },
            AstType::Unit | AstType::Boolean => Ok(()),
        }
    }

    fn check_all_functions(&self, module: &Module, file_id: FileId) -> Result<(), TypeError> {
        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                self.check_function(func, file_id)?;
            }
        }
        Ok(())
    }

    fn check_function(&self, func: &Function, file_id: FileId) -> Result<(), TypeError> {
        let mut env = TypeEnvironment::new();

        for param in &func.parameters {
            env.declare_variable(param.name.clone(), param.param_type.clone());
        }

        for statement in &func.body.statements {
            env = self.check_statement(statement, env, &func.name, file_id)?;
        }

        Ok(())
    }

    fn check_statement(
        &self,
        statement: &Statement,
        mut env: TypeEnvironment,
        function_name: &str,
        file_id: FileId,
    ) -> Result<TypeEnvironment, TypeError> {
        match statement {
            Statement::Injection(expr) => {
                self.check_expression(expr, &env, file_id)?;
                Ok(env)
            }
            Statement::Assignment {
                variable,
                expression,
                ..
            } => {
                let expr_type = self.check_expression(expression, &env, file_id)?;
                env.declare_variable(variable.clone(), expr_type);
                Ok(env)
            }
            Statement::VariableAssignment {
                variable,
                expression,
                span,
            } => {
                let expr_type = self.check_expression(expression, &env, file_id)?;
                let existing_type =
                    env.lookup_variable(variable)
                        .ok_or_else(|| TypeError::UnknownVariable {
                            name: variable.clone(),
                            span: *span,
                            file_id,
                        })?;

                if !self.types_equal(&expr_type, &existing_type) {
                    return Err(TypeError::TypeMismatch {
                        expected: format!("{}", existing_type),
                        found: format!("{}", expr_type),
                        span: expression.span(),
                        file_id,
                    });
                }

                Ok(env)
            }
            Statement::ExpressionStatement(expr) => {
                self.check_expression(expr, &env, file_id)?;
                Ok(env)
            }
            Statement::If {
                condition,
                body,
                span: _,
            } => {
                let cond_type = self.check_expression(condition, &env, file_id)?;
                if !matches!(cond_type, AstType::Boolean) {
                    return Err(TypeError::TypeMismatch {
                        expected: "Boolean".to_string(),
                        found: format!("{}", cond_type),
                        span: condition.span(),
                        file_id,
                    });
                }

                let mut child_env = env.create_child();
                for stmt in body {
                    child_env = self.check_statement(stmt, child_env, function_name, file_id)?;
                }
                Ok(env)
            }
            Statement::While {
                condition,
                body,
                span: _,
            } => {
                let cond_type = self.check_expression(condition, &env, file_id)?;
                if !matches!(cond_type, AstType::Boolean) {
                    return Err(TypeError::TypeMismatch {
                        expected: "Boolean".to_string(),
                        found: format!("{}", cond_type),
                        span: condition.span(),
                        file_id,
                    });
                }

                let mut child_env = env.create_child();
                for stmt in body {
                    child_env = self.check_statement(stmt, child_env, function_name, file_id)?;
                }
                Ok(env)
            }
            Statement::Return(expr) => {
                let return_type = self.check_expression(expr, &env, file_id)?;
                let expected_type = &self
                    .function_signatures
                    .get(function_name)
                    .expect("Function signature not found")
                    .return_type;

                if return_type != *expected_type {
                    return Err(TypeError::ReturnTypeMismatch {
                        function: function_name.to_string(),
                        expected: format!("{}", expected_type),
                        found: format!("{}", return_type),
                        span: expr.span(),
                        file_id,
                    });
                }
                Ok(env)
            }
        }
    }

    fn check_expression(
        &self,
        expression: &Expression,
        env: &TypeEnvironment,
        file_id: FileId,
    ) -> Result<AstType, TypeError> {
        match expression {
            Expression::Call {
                function,
                arguments,
                span,
            } => {
                let func_sig = self.function_signatures.get(function).ok_or_else(|| {
                    TypeError::UnknownFunction {
                        name: function.clone(),
                        span: *span,
                        file_id,
                    }
                })?;

                if arguments.len() != func_sig.parameters.len() {
                    return Err(TypeError::ArgumentCountMismatch {
                        function: function.clone(),
                        expected: func_sig.parameters.len(),
                        found: arguments.len(),
                        span: *span,
                        file_id,
                    });
                }

                for (arg, param) in arguments.iter().zip(&func_sig.parameters) {
                    match arg {
                        Expression::Placeholder { .. } => {}
                        _ => {
                            let arg_type = self.check_expression(arg, env, file_id)?;
                            if !self.types_equal(&arg_type, &param.param_type) {
                                return Err(TypeError::ArgumentTypeMismatch {
                                    function: function.clone(),
                                    parameter: param.name.clone(),
                                    expected: format!("{}", param.param_type),
                                    found: format!("{}", arg_type),
                                    span: arg.span(),
                                    file_id,
                                });
                            }
                        }
                    }
                }

                Ok(func_sig.return_type.clone())
            }
            Expression::Variable { name, span } => {
                env.lookup_variable(name)
                    .ok_or_else(|| TypeError::UnknownVariable {
                        name: name.clone(),
                        span: *span,
                        file_id,
                    })
            }
            Expression::StringLiteral { .. } => Ok(AstType::Named("String".to_string())),
            Expression::BooleanLiteral { .. } => Ok(AstType::Boolean),
            Expression::Placeholder { span } => Err(TypeError::TypeMismatch {
                expected: "concrete type".to_string(),
                found: "placeholder".to_string(),
                span: *span,
                file_id,
            }),
            Expression::Select(select_expr) => {
                if select_expr.clauses.is_empty() {
                    return Err(TypeError::TypeMismatch {
                        expected: "non-empty select".to_string(),
                        found: "empty select".to_string(),
                        span: select_expr.span,
                        file_id,
                    });
                }

                let first_clause = &select_expr.clauses[0];
                let first_result_type =
                    self.check_expression(&first_clause.expression_to_run, env, file_id)?;
                let mut first_clause_env = env.create_child();
                first_clause_env
                    .declare_variable(first_clause.result_variable.clone(), first_result_type);
                let first_type = self.check_expression(
                    &first_clause.expression_next,
                    &first_clause_env,
                    file_id,
                )?;

                for (i, clause) in select_expr.clauses.iter().enumerate().skip(1) {
                    let result_type =
                        self.check_expression(&clause.expression_to_run, env, file_id)?;
                    let mut clause_env = env.create_child();
                    clause_env.declare_variable(clause.result_variable.clone(), result_type);
                    let clause_type =
                        self.check_expression(&clause.expression_next, &clause_env, file_id)?;
                    if !self.types_equal(&first_type, &clause_type) {
                        return Err(TypeError::SelectBranchTypeMismatch {
                            expected: format!("{}", first_type),
                            found: format!("{}", clause_type),
                            branch_index: i,
                            span: clause.span,
                            file_id,
                        });
                    }
                }

                Ok(first_type)
            }
        }
    }

    fn types_equal(&self, type1: &AstType, type2: &AstType) -> bool {
        match (type1, type2) {
            (AstType::Named(name1), AstType::Named(name2)) => name1 == name2,
            (AstType::Unit, AstType::Unit) => true,
            (AstType::Boolean, AstType::Boolean) => true,
            _ => false,
        }
    }
}

impl TypeEnvironment {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parent: None,
        }
    }

    fn create_child(&self) -> Self {
        Self {
            variables: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    fn declare_variable(&mut self, name: String, ast_type: AstType) {
        self.variables.insert(name, ast_type);
    }

    fn lookup_variable(&self, name: &str) -> Option<AstType> {
        if let Some(ast_type) = self.variables.get(name) {
            Some(ast_type.clone())
        } else if let Some(ref parent) = self.parent {
            parent.lookup_variable(name)
        } else {
            None
        }
    }
}
