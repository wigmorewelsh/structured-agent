use crate::ast::{Definition, Expression, Function, Module, Parameter, Statement, Type as AstType};
use crate::typecheck::error::TypeError;
use std::collections::HashMap;

#[derive(Debug)]
pub struct TypeChecker {
    function_signatures: HashMap<String, FunctionSignature>,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    name: String,
    parameters: Vec<Parameter>,
    return_type: AstType,
}

#[derive(Debug, Clone)]
struct TypeEnvironment {
    variables: HashMap<String, AstType>,
    parent: Option<Box<TypeEnvironment>>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            function_signatures: HashMap::new(),
        }
    }

    pub fn check_module(&mut self, module: &Module) -> Result<(), TypeError> {
        self.collect_function_signatures(module)?;
        self.check_all_functions(module)?;
        Ok(())
    }

    fn collect_function_signatures(&mut self, module: &Module) -> Result<(), TypeError> {
        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    self.validate_type(&func.return_type)?;
                    for param in &func.parameters {
                        self.validate_type(&param.param_type)?;
                    }

                    let signature = FunctionSignature {
                        name: func.name.clone(),
                        parameters: func.parameters.clone(),
                        return_type: func.return_type.clone(),
                    };
                    self.function_signatures
                        .insert(func.name.clone(), signature);
                }
                Definition::ExternalFunction(ext_func) => {
                    self.validate_type(&ext_func.return_type)?;
                    for param in &ext_func.parameters {
                        self.validate_type(&param.param_type)?;
                    }

                    let signature = FunctionSignature {
                        name: ext_func.name.clone(),
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

    fn validate_type(&self, ast_type: &AstType) -> Result<(), TypeError> {
        match ast_type {
            AstType::Named(name) => match name.as_str() {
                "String" | "Boolean" | "Context" => Ok(()),
                _ => Err(TypeError::UnsupportedType(name.clone())),
            },
            AstType::Unit | AstType::Boolean => Ok(()),
        }
    }

    fn check_all_functions(&self, module: &Module) -> Result<(), TypeError> {
        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                self.check_function(func)?;
            }
        }
        Ok(())
    }

    fn check_function(&self, func: &Function) -> Result<(), TypeError> {
        let mut env = TypeEnvironment::new();

        for param in &func.parameters {
            env.declare_variable(param.name.clone(), param.param_type.clone());
        }

        for statement in &func.body.statements {
            env = self.check_statement(statement, env, &func.name)?;
        }

        Ok(())
    }

    fn check_statement(
        &self,
        statement: &Statement,
        mut env: TypeEnvironment,
        function_name: &str,
    ) -> Result<TypeEnvironment, TypeError> {
        match statement {
            Statement::Injection(expr) => {
                self.check_expression(expr, &env)?;
                Ok(env)
            }
            Statement::Assignment {
                variable,
                expression,
            } => {
                let expr_type = self.check_expression(expression, &env)?;
                env.declare_variable(variable.clone(), expr_type);
                Ok(env)
            }
            Statement::VariableAssignment {
                variable,
                expression,
            } => {
                let expr_type = self.check_expression(expression, &env)?;

                if let Some(var_type) = env.get(variable) {
                    if !self.types_equal(&expr_type, &var_type) {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{}", var_type),
                            found: format!("{}", expr_type),
                            location: format!("assignment to variable {}", variable),
                        });
                    }
                } else {
                    return Err(TypeError::UnknownVariable(variable.clone()));
                }
                Ok(env)
            }
            Statement::ExpressionStatement(expr) => {
                self.check_expression(expr, &env)?;
                Ok(env)
            }
            Statement::If { condition, body } => {
                let condition_type = self.check_expression(condition, &env)?;

                if !matches!(condition_type, AstType::Boolean) {
                    return Err(TypeError::TypeMismatch {
                        expected: "Boolean".to_string(),
                        found: format!("{}", condition_type),
                        location: "if condition".to_string(),
                    });
                }

                let mut body_env = env.create_child();
                for stmt in body {
                    body_env = self.check_statement(stmt, body_env, function_name)?;
                }
                Ok(env)
            }
            Statement::While { condition, body } => {
                let condition_type = self.check_expression(condition, &env)?;

                if !matches!(condition_type, AstType::Boolean) {
                    return Err(TypeError::TypeMismatch {
                        expected: "Boolean".to_string(),
                        found: format!("{}", condition_type),
                        location: "while condition".to_string(),
                    });
                }

                let mut body_env = env.create_child();
                for stmt in body {
                    body_env = self.check_statement(stmt, body_env, function_name)?;
                }
                Ok(env)
            }
            Statement::Return(expr) => {
                let expr_type = self.check_expression(expr, &env)?;
                let func_sig = self
                    .function_signatures
                    .get(function_name)
                    .ok_or_else(|| TypeError::UnknownFunction(function_name.to_string()))?;

                if !self.types_equal(&expr_type, &func_sig.return_type) {
                    return Err(TypeError::ReturnTypeMismatch {
                        function: function_name.to_string(),
                        expected: format!("{}", func_sig.return_type),
                        found: format!("{}", expr_type),
                    });
                }
                Ok(env)
            }
        }
    }

    fn check_expression(
        &self,
        expr: &Expression,
        env: &TypeEnvironment,
    ) -> Result<AstType, TypeError> {
        match expr {
            Expression::Call {
                function,
                arguments,
            } => {
                let func_sig = self
                    .function_signatures
                    .get(function)
                    .ok_or_else(|| TypeError::UnknownFunction(function.clone()))?;

                if arguments.len() != func_sig.parameters.len() {
                    return Err(TypeError::ArgumentCountMismatch {
                        function: function.clone(),
                        expected: func_sig.parameters.len(),
                        found: arguments.len(),
                    });
                }

                for (_i, (arg, param)) in arguments.iter().zip(&func_sig.parameters).enumerate() {
                    match arg {
                        Expression::Placeholder => {}
                        _ => {
                            let arg_type = self.check_expression(arg, env)?;
                            if !self.types_equal(&arg_type, &param.param_type) {
                                return Err(TypeError::ArgumentTypeMismatch {
                                    function: function.clone(),
                                    parameter: param.name.clone(),
                                    expected: format!("{}", param.param_type),
                                    found: format!("{}", arg_type),
                                });
                            }
                        }
                    }
                }

                Ok(func_sig.return_type.clone())
            }
            Expression::Variable(name) => env
                .get(name)
                .ok_or_else(|| TypeError::UnknownVariable(name.clone())),
            Expression::StringLiteral(_) => Ok(AstType::Named("String".to_string())),
            Expression::BooleanLiteral(_) => Ok(AstType::Boolean),
            Expression::Placeholder => Err(TypeError::TypeMismatch {
                expected: "concrete type".to_string(),
                found: "placeholder".to_string(),
                location: "standalone placeholder".to_string(),
            }),
            Expression::Select(select_expr) => {
                if select_expr.clauses.is_empty() {
                    return Err(TypeError::TypeMismatch {
                        expected: "non-empty select".to_string(),
                        found: "empty select".to_string(),
                        location: "select expression".to_string(),
                    });
                }

                let first_clause = &select_expr.clauses[0];
                let first_type = self.check_expression(&first_clause.expression_to_run, env)?;

                for (i, clause) in select_expr.clauses.iter().enumerate().skip(1) {
                    let clause_type = self.check_expression(&clause.expression_to_run, env)?;
                    if !self.types_equal(&first_type, &clause_type) {
                        return Err(TypeError::SelectBranchTypeMismatch {
                            expected: format!("{}", first_type),
                            found: format!("{}", clause_type),
                            branch_index: i,
                        });
                    }
                }

                for clause in &select_expr.clauses {
                    let mut clause_env = env.create_child();
                    clause_env.declare_variable(clause.result_variable.clone(), first_type.clone());
                    self.check_expression(&clause.expression_next, &clause_env)?;
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

    fn get(&self, name: &str) -> Option<AstType> {
        if let Some(ast_type) = self.variables.get(name) {
            Some(ast_type.clone())
        } else if let Some(ref parent) = self.parent {
            parent.get(name)
        } else {
            None
        }
    }
}
