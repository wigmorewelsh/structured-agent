use crate::ast::{
    Definition, Expression, Function, Module, Parameter, SelectClause, Statement, Type as AstType,
};
use crate::typecheck::error::TypeError;
use crate::typed_ast;
use crate::types::{FileId, Span, Spanned};
use std::collections::HashMap;

pub type ModuleVisibility = HashMap<String, bool>;
pub type AliasToQualified = HashMap<String, String>;

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionKind {
    Bytecode,
    External,
}

#[derive(Debug)]
pub struct TypeChecker {
    function_signatures: HashMap<String, FunctionSignature>,
    struct_definitions: HashMap<String, Vec<(String, AstType)>>,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    parameters: Vec<Parameter>,
    return_type: AstType,
    kind: FunctionKind,
    type_params: Vec<String>,
}

#[derive(Debug, Clone)]
struct TypeEnvironment {
    variables: HashMap<String, (AstType, Span)>,
    parent: Option<Box<TypeEnvironment>>,
}

struct CheckContext<'a> {
    file_id: FileId,
    alias_map: &'a HashMap<String, String>,
    module_visibility: &'a ModuleVisibility,
    alias_to_qualified: &'a AliasToQualified,
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
            struct_definitions: HashMap::new(),
        }
    }

    pub fn check_module(&mut self, module: &Module, file_id: FileId) -> Result<(), TypeError> {
        self.check_module_with_external_sigs(
            module,
            file_id,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        )
        .map(|_| ())
    }

    pub fn check_module_with_external_sigs(
        &mut self,
        module: &Module,
        file_id: FileId,
        external_sigs: &HashMap<String, ExternalSig>,
        module_visibility: &ModuleVisibility,
        sig_definitions: &HashMap<String, Vec<crate::ast::SigFunction>>,
    ) -> Result<(typed_ast::Module, HashMap<String, FunctionKind>), TypeError> {
        for (name, sig) in external_sigs {
            self.function_signatures.insert(
                name.clone(),
                FunctionSignature {
                    parameters: sig.parameters.clone(),
                    return_type: sig.return_type.clone(),
                    kind: sig.kind.clone(),
                    type_params: sig.type_params.clone(),
                },
            );
        }

        self.register_param_sigs(module, sig_definitions, external_sigs);

        self.collect_function_signatures(module, file_id)?;

        let alias_map = Self::build_alias_map(module);
        let alias_to_qualified = Self::build_alias_to_qualified(module, module_visibility);

        let ctx = CheckContext {
            file_id,
            alias_map: &alias_map,
            module_visibility,
            alias_to_qualified: &alias_to_qualified,
        };

        let mut typed_definitions = Vec::new();
        for definition in &module.definitions {
            let typed_def = match definition {
                Definition::Function(func) => {
                    typed_ast::Definition::Function(self.check_function(func, &ctx)?)
                }
                Definition::ExternalFunction(f) => {
                    typed_ast::Definition::ExternalFunction(f.clone())
                }
                Definition::Struct(s) => typed_ast::Definition::Struct(s.clone()),
                Definition::Use {
                    path,
                    alias,
                    is_pub,
                    span,
                } => typed_ast::Definition::Use {
                    path: path.clone(),
                    alias: alias.clone(),
                    is_pub: *is_pub,
                    span: *span,
                },
                Definition::ModuleHeader { name, params, span } => {
                    typed_ast::Definition::ModuleHeader {
                        name: name.clone(),
                        params: params.clone(),
                        span: *span,
                    }
                }
                Definition::ModuleBinding {
                    name,
                    sig_path,
                    impl_path,
                    span,
                } => typed_ast::Definition::ModuleBinding {
                    name: name.clone(),
                    sig_path: sig_path.clone(),
                    impl_path: impl_path.clone(),
                    span: *span,
                },
                Definition::WiringSite { name, args, span } => typed_ast::Definition::WiringSite {
                    name: name.clone(),
                    args: args.clone(),
                    span: *span,
                },
                Definition::Signature {
                    name,
                    functions,
                    span,
                } => typed_ast::Definition::Signature {
                    name: name.clone(),
                    functions: functions.clone(),
                    span: *span,
                },
            };
            typed_definitions.push(typed_def);
        }

        let typed_module = typed_ast::Module {
            definitions: typed_definitions,
            span: module.span,
            file_id,
        };

        Ok((typed_module, self.function_kinds()))
    }

    fn register_param_sigs(
        &mut self,
        module: &Module,
        sig_definitions: &HashMap<String, Vec<crate::ast::SigFunction>>,
        external_sigs: &HashMap<String, ExternalSig>,
    ) {
        let params = module.definitions.iter().find_map(|def| {
            if let Definition::ModuleHeader { params, .. } = def {
                Some(params)
            } else {
                None
            }
        });

        let params = match params {
            Some(p) => p,
            None => return,
        };

        for param in params {
            if param.path.len() < 2 {
                continue;
            }
            let concrete_module = &param.path[0];
            let sig_name = param.path.last().unwrap();

            let fn_names: Vec<(String, Vec<crate::ast::Parameter>, crate::ast::Type)> =
                if let Some(fns) = sig_definitions.get(sig_name) {
                    fns.iter()
                        .map(|f| (f.name.clone(), f.parameters.clone(), f.return_type.clone()))
                        .collect()
                } else {
                    external_sigs
                        .iter()
                        .filter_map(|(k, sig)| {
                            k.strip_prefix(&format!("{}::", concrete_module))
                                .map(|fn_name| {
                                    (
                                        fn_name.to_string(),
                                        sig.parameters.clone(),
                                        sig.return_type.clone(),
                                    )
                                })
                        })
                        .collect()
                };

            for (fn_name, fn_params, ret_type) in fn_names {
                let key = format!("{}::{}", param.name, fn_name);
                self.function_signatures.insert(
                    key,
                    FunctionSignature {
                        parameters: fn_params,
                        return_type: ret_type,
                        kind: FunctionKind::External,
                        type_params: vec![],
                    },
                );
            }
        }
    }

    fn collect_function_signatures(
        &mut self,
        module: &Module,
        file_id: FileId,
    ) -> Result<(), TypeError> {
        for definition in &module.definitions {
            if let Definition::Struct(struct_def) = definition {
                let fields = struct_def
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), f.field_type.clone()))
                    .collect();
                self.struct_definitions
                    .insert(struct_def.name.clone(), fields);
            }
        }

        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    let resolved_return = self.resolve_type(&func.return_type);
                    self.validate_type_with_params(
                        &resolved_return,
                        func.span,
                        file_id,
                        &func.type_params,
                    )?;
                    let resolved_params: Vec<_> = func
                        .parameters
                        .iter()
                        .map(|p| crate::ast::Parameter {
                            name: p.name.clone(),
                            param_type: self.resolve_type(&p.param_type),
                            span: p.span,
                        })
                        .collect();
                    for (resolved_param, orig_param) in resolved_params.iter().zip(&func.parameters)
                    {
                        self.validate_type_with_params(
                            &resolved_param.param_type,
                            orig_param.span,
                            file_id,
                            &func.type_params,
                        )?;
                    }
                    self.function_signatures.insert(
                        func.name.clone(),
                        FunctionSignature {
                            parameters: resolved_params,
                            return_type: resolved_return,
                            kind: FunctionKind::Bytecode,
                            type_params: func.type_params.clone(),
                        },
                    );
                }
                Definition::ExternalFunction(ext_func) => {
                    self.validate_type_with_params(
                        &ext_func.return_type,
                        ext_func.span,
                        file_id,
                        &ext_func.type_params,
                    )?;
                    for param in &ext_func.parameters {
                        self.validate_type_with_params(
                            &param.param_type,
                            param.span,
                            file_id,
                            &ext_func.type_params,
                        )?;
                    }
                    let resolved_params: Vec<_> = ext_func
                        .parameters
                        .iter()
                        .map(|p| crate::ast::Parameter {
                            name: p.name.clone(),
                            param_type: self.resolve_type(&p.param_type),
                            span: p.span,
                        })
                        .collect();
                    self.function_signatures.insert(
                        ext_func.name.clone(),
                        FunctionSignature {
                            parameters: resolved_params,
                            return_type: self.resolve_type(&ext_func.return_type),
                            kind: FunctionKind::External,
                            type_params: ext_func.type_params.clone(),
                        },
                    );
                }
                Definition::Struct(_)
                | Definition::Use { .. }
                | Definition::ModuleHeader { .. }
                | Definition::ModuleBinding { .. }
                | Definition::WiringSite { .. }
                | Definition::Signature { .. } => {}
            }
        }
        Ok(())
    }

    fn build_alias_map(module: &Module) -> HashMap<String, String> {
        module
            .definitions
            .iter()
            .filter_map(|def| {
                if let Definition::Use {
                    path,
                    alias: Some(a),
                    ..
                } = def
                {
                    Some((a.clone(), path.last().cloned().unwrap_or_default()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn build_alias_to_qualified(
        module: &Module,
        module_visibility: &ModuleVisibility,
    ) -> AliasToQualified {
        let mut map = AliasToQualified::new();

        for def in &module.definitions {
            let Definition::Use { path, alias, .. } = def else {
                continue;
            };
            if path.len() < 2 {
                continue;
            }

            let dep_module = &path[0];
            let fn_name = path.last().unwrap();
            let qualified = format!("{}::{}", dep_module, fn_name);

            if module_visibility.contains_key(&qualified) {
                let key = alias.clone().unwrap_or_else(|| fn_name.clone());
                map.insert(key, qualified);
            }
        }

        map
    }

    fn resolve_type(&self, t: &AstType) -> AstType {
        match t {
            AstType::Generic(name) if self.struct_definitions.contains_key(name) => {
                AstType::Struct(name.clone())
            }
            AstType::List(inner) => AstType::List(Box::new(self.resolve_type(inner))),
            AstType::Option(inner) => AstType::Option(Box::new(self.resolve_type(inner))),
            other => other.clone(),
        }
    }

    fn validate_type_with_params(
        &self,
        ast_type: &AstType,
        span: Span,
        file_id: FileId,
        type_params: &[String],
    ) -> Result<(), TypeError> {
        match ast_type {
            AstType::Unit | AstType::Boolean | AstType::String | AstType::Int => Ok(()),
            AstType::Generic(name) => {
                if type_params.contains(name) {
                    Ok(())
                } else {
                    Err(TypeError::UnsupportedType {
                        type_name: name.clone(),
                        span,
                        file_id,
                    })
                }
            }
            AstType::List(inner) | AstType::Option(inner) => {
                self.validate_type_with_params(inner, span, file_id, type_params)
            }
            AstType::Struct(name) => {
                if self.struct_definitions.contains_key(name) {
                    Ok(())
                } else {
                    Err(TypeError::UnsupportedType {
                        type_name: name.clone(),
                        span,
                        file_id,
                    })
                }
            }
        }
    }

    fn unify_type(
        formal: &AstType,
        actual: &AstType,
        subst: &mut HashMap<String, AstType>,
    ) -> bool {
        match formal {
            AstType::Generic(name) => {
                if let Some(bound) = subst.get(name) {
                    bound == actual
                } else {
                    subst.insert(name.clone(), actual.clone());
                    true
                }
            }
            AstType::List(inner_formal) => {
                if let AstType::List(inner_actual) = actual {
                    Self::unify_type(inner_formal, inner_actual, subst)
                } else {
                    false
                }
            }
            AstType::Option(inner_formal) => {
                if let AstType::Option(inner_actual) = actual {
                    Self::unify_type(inner_formal, inner_actual, subst)
                } else {
                    false
                }
            }
            _ => formal == actual,
        }
    }

    fn apply_subst(ty: &AstType, subst: &HashMap<String, AstType>) -> AstType {
        match ty {
            AstType::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            AstType::List(inner) => AstType::List(Box::new(Self::apply_subst(inner, subst))),
            AstType::Option(inner) => AstType::Option(Box::new(Self::apply_subst(inner, subst))),
            other => other.clone(),
        }
    }

    fn check_function(
        &self,
        func: &Function,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Function, TypeError> {
        let mut env = TypeEnvironment::new();
        for param in &func.parameters {
            env.declare_variable(
                param.name.clone(),
                self.resolve_type(&param.param_type),
                param.span,
            );
        }
        let mut typed_stmts = Vec::new();
        for statement in &func.body.statements {
            let (typed_stmt, new_env) = self.check_statement(statement, env, &func.name, ctx)?;
            typed_stmts.push(typed_stmt);
            env = new_env;
        }
        Ok(typed_ast::Function {
            name: func.name.clone(),
            parameters: func.parameters.clone(),
            return_type: func.return_type.clone(),
            body: typed_ast::FunctionBody {
                statements: typed_stmts,
                span: func.body.span,
            },
            documentation: func.documentation.clone(),
            is_pub: func.is_pub,
            span: func.span,
        })
    }

    fn check_statement(
        &self,
        statement: &Statement,
        mut env: TypeEnvironment,
        function_name: &str,
        ctx: &CheckContext,
    ) -> Result<(typed_ast::Statement, TypeEnvironment), TypeError> {
        match statement {
            Statement::Injection(expr) => {
                let typed_expr = self.check_expression(expr, &env, ctx)?;
                Ok((typed_ast::Statement::Injection(typed_expr), env))
            }
            Statement::Assignment {
                variable,
                expression,
                span,
            } => {
                let typed_expr = self.check_expression(expression, &env, ctx)?;
                let expr_type = typed_expr.ty().clone();
                env.declare_variable(variable.clone(), expr_type, expression.span());
                Ok((
                    typed_ast::Statement::Assignment {
                        variable: variable.clone(),
                        expression: typed_expr,
                        span: *span,
                    },
                    env,
                ))
            }
            Statement::VariableAssignment {
                variable,
                expression,
                span,
            } => {
                let typed_expr = self.check_expression(expression, &env, ctx)?;
                let expr_type = typed_expr.ty().clone();
                let (existing_type, declaration_span) = env
                    .lookup_variable_with_span(variable)
                    .ok_or_else(|| TypeError::UnknownVariable {
                        name: variable.clone(),
                        span: *span,
                        file_id: ctx.file_id,
                    })?;

                if expr_type != existing_type {
                    return Err(TypeError::VariableTypeMismatch {
                        variable: variable.clone(),
                        expected: format!("{}", existing_type),
                        found: format!("{}", expr_type),
                        span: expression.span(),
                        declaration_span,
                        file_id: ctx.file_id,
                    });
                }
                Ok((
                    typed_ast::Statement::VariableAssignment {
                        variable: variable.clone(),
                        expression: typed_expr,
                        span: *span,
                    },
                    env,
                ))
            }
            Statement::ExpressionStatement(expr) => {
                let typed_expr = self.check_expression(expr, &env, ctx)?;
                Ok((typed_ast::Statement::ExpressionStatement(typed_expr), env))
            }
            Statement::If {
                condition,
                body,
                else_body,
                span,
            } => {
                let typed_condition = self.check_boolean_condition(condition, &env, ctx)?;
                let typed_body = self.check_block(body, env.create_child(), function_name, ctx)?;
                let typed_else = if let Some(else_stmts) = else_body {
                    Some(self.check_block(else_stmts, env.create_child(), function_name, ctx)?)
                } else {
                    None
                };
                Ok((
                    typed_ast::Statement::If {
                        condition: typed_condition,
                        body: typed_body,
                        else_body: typed_else,
                        span: *span,
                    },
                    env,
                ))
            }
            Statement::While {
                condition,
                body,
                span,
            } => {
                let typed_condition = self.check_boolean_condition(condition, &env, ctx)?;
                let typed_body = self.check_block(body, env.create_child(), function_name, ctx)?;
                Ok((
                    typed_ast::Statement::While {
                        condition: typed_condition,
                        body: typed_body,
                        span: *span,
                    },
                    env,
                ))
            }
            Statement::Return(expr) => {
                let typed_expr = self.check_expression(expr, &env, ctx)?;
                let expected_type = self
                    .function_signatures
                    .get(function_name)
                    .expect("function signature not found")
                    .return_type
                    .clone();

                if *typed_expr.ty() != expected_type {
                    return Err(TypeError::ReturnTypeMismatch {
                        function: function_name.to_string(),
                        expected: format!("{}", expected_type),
                        found: format!("{}", typed_expr.ty()),
                        span: expr.span(),
                        file_id: ctx.file_id,
                    });
                }
                Ok((typed_ast::Statement::Return(typed_expr), env))
            }
        }
    }

    fn check_boolean_condition(
        &self,
        condition: &Expression,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let typed_cond = self.check_expression(condition, env, ctx)?;
        if matches!(typed_cond.ty(), AstType::Boolean) {
            Ok(typed_cond)
        } else {
            Err(TypeError::TypeMismatch {
                expected: "Boolean".to_string(),
                found: format!("{}", typed_cond.ty()),
                span: condition.span(),
                file_id: ctx.file_id,
            })
        }
    }

    fn check_block(
        &self,
        stmts: &[Statement],
        mut env: TypeEnvironment,
        function_name: &str,
        ctx: &CheckContext,
    ) -> Result<Vec<typed_ast::Statement>, TypeError> {
        let mut typed_stmts = Vec::new();
        for stmt in stmts {
            let (typed_stmt, new_env) = self.check_statement(stmt, env, function_name, ctx)?;
            typed_stmts.push(typed_stmt);
            env = new_env;
        }
        Ok(typed_stmts)
    }

    fn check_expression(
        &self,
        expression: &Expression,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        match expression {
            Expression::Call {
                function,
                arguments,
                span,
            } => self.check_call(function, arguments, *span, env, ctx),
            Expression::Variable { name, span } => {
                let ty = env
                    .lookup_variable(name)
                    .ok_or_else(|| TypeError::UnknownVariable {
                        name: name.clone(),
                        span: *span,
                        file_id: ctx.file_id,
                    })?;
                Ok(typed_ast::Expression::Variable {
                    name: name.clone(),
                    ty,
                    span: *span,
                })
            }
            Expression::StringLiteral { value, span } => Ok(typed_ast::Expression::StringLiteral {
                value: value.clone(),
                ty: AstType::String,
                span: *span,
            }),
            Expression::BooleanLiteral { value, span } => {
                Ok(typed_ast::Expression::BooleanLiteral {
                    value: *value,
                    ty: AstType::Boolean,
                    span: *span,
                })
            }
            Expression::IntLiteral { value, span } => Ok(typed_ast::Expression::IntLiteral {
                value: *value,
                ty: AstType::Int,
                span: *span,
            }),
            Expression::UnitLiteral { span } => Ok(typed_ast::Expression::UnitLiteral {
                ty: AstType::Unit,
                span: *span,
            }),
            Expression::Placeholder { span } => Err(TypeError::TypeMismatch {
                expected: "concrete type".to_string(),
                found: "placeholder".to_string(),
                span: *span,
                file_id: ctx.file_id,
            }),
            Expression::ListLiteral { elements, span } => {
                self.check_list_literal(elements, *span, env, ctx)
            }
            Expression::Select(select_expr) => {
                self.check_select(&select_expr.clauses, select_expr.span, env, ctx)
            }
            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                span,
            } => self.check_if_else_expression(condition, then_expr, else_expr, *span, env, ctx),
            Expression::StructLiteral {
                struct_name,
                fields,
                span,
            } => self.check_struct_literal(struct_name, fields, *span, env, ctx),
            Expression::FieldAccess { base, field, span } => {
                self.check_field_access(base, field, *span, env, ctx)
            }
        }
    }

    fn check_call(
        &self,
        function: &str,
        arguments: &[Expression],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let resolved = ctx
            .alias_map
            .get(function)
            .map(String::as_str)
            .unwrap_or(function);

        let qualified_for_vis = ctx
            .alias_to_qualified
            .get(function)
            .or_else(|| ctx.alias_to_qualified.get(resolved))
            .map(String::as_str)
            .unwrap_or(resolved);

        self.check_visibility(qualified_for_vis, resolved, span, ctx)?;

        let (kind, return_type, parameters, type_params) = {
            let func_sig = self.function_signatures.get(resolved).ok_or_else(|| {
                TypeError::UnknownFunction {
                    name: function.to_string(),
                    span,
                    file_id: ctx.file_id,
                }
            })?;
            (
                func_sig.kind.clone(),
                func_sig.return_type.clone(),
                func_sig.parameters.clone(),
                func_sig.type_params.clone(),
            )
        };

        if arguments.len() != parameters.len() {
            return Err(TypeError::ArgumentCountMismatch {
                function: function.to_string(),
                expected: parameters.len(),
                found: arguments.len(),
                span,
                file_id: ctx.file_id,
            });
        }

        let mut typed_args = Vec::new();

        if type_params.is_empty() {
            for (arg, param) in arguments.iter().zip(&parameters) {
                if matches!(arg, Expression::Placeholder { .. }) {
                    typed_args.push(typed_ast::Expression::Placeholder {
                        ty: param.param_type.clone(),
                        span: arg.span(),
                    });
                    continue;
                }
                let typed_arg = self.check_expression(arg, env, ctx)?;
                if typed_arg.ty() != &param.param_type {
                    return Err(TypeError::ArgumentTypeMismatch {
                        function: function.to_string(),
                        parameter: param.name.clone(),
                        expected: format!("{}", param.param_type),
                        found: format!("{}", typed_arg.ty()),
                        span: arg.span(),
                        file_id: ctx.file_id,
                    });
                }
                typed_args.push(typed_arg);
            }

            Ok(typed_ast::Expression::Call {
                function: function.to_string(),
                resolved: resolved.to_string(),
                kind,
                arguments: typed_args,
                ty: return_type,
                span,
            })
        } else {
            let mut subst: HashMap<String, AstType> = HashMap::new();
            for (arg, param) in arguments.iter().zip(&parameters) {
                if matches!(arg, Expression::Placeholder { .. }) {
                    typed_args.push(typed_ast::Expression::Placeholder {
                        ty: param.param_type.clone(),
                        span: arg.span(),
                    });
                    continue;
                }
                let typed_arg = self.check_expression(arg, env, ctx)?;
                if !Self::unify_type(&param.param_type, typed_arg.ty(), &mut subst) {
                    let expected = Self::apply_subst(&param.param_type, &subst);
                    return Err(TypeError::ArgumentTypeMismatch {
                        function: function.to_string(),
                        parameter: param.name.clone(),
                        expected: format!("{}", expected),
                        found: format!("{}", typed_arg.ty()),
                        span: arg.span(),
                        file_id: ctx.file_id,
                    });
                }
                typed_args.push(typed_arg);
            }

            let resolved_return = Self::apply_subst(&return_type, &subst);
            Ok(typed_ast::Expression::Call {
                function: function.to_string(),
                resolved: resolved.to_string(),
                kind,
                arguments: typed_args,
                ty: resolved_return,
                span,
            })
        }
    }

    fn check_visibility(
        &self,
        qualified_for_vis: &str,
        resolved: &str,
        span: Span,
        ctx: &CheckContext,
    ) -> Result<(), TypeError> {
        if ctx.module_visibility.is_empty() {
            return Ok(());
        }

        let name_to_check = if qualified_for_vis.contains("::") {
            qualified_for_vis
        } else if resolved.contains("::") {
            resolved
        } else {
            return Ok(());
        };

        let is_visible = ctx
            .module_visibility
            .get(name_to_check)
            .copied()
            .unwrap_or(true);
        if is_visible {
            Ok(())
        } else {
            Err(TypeError::PrivateFunction {
                name: name_to_check.to_string(),
                span,
                file_id: ctx.file_id,
            })
        }
    }

    fn check_list_literal(
        &self,
        elements: &[Expression],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        if elements.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "non-empty list or type annotation".to_string(),
                found: "empty list".to_string(),
                span,
                file_id: ctx.file_id,
            });
        }

        let typed_first = self.check_expression(&elements[0], env, ctx)?;
        let first_type = typed_first.ty().clone();
        let mut typed_elements = vec![typed_first];

        for elem in elements.iter().skip(1) {
            let typed_elem = self.check_expression(elem, env, ctx)?;
            if *typed_elem.ty() != first_type {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{}", first_type),
                    found: format!("{}", typed_elem.ty()),
                    span: elem.span(),
                    file_id: ctx.file_id,
                });
            }
            typed_elements.push(typed_elem);
        }

        Ok(typed_ast::Expression::ListLiteral {
            elements: typed_elements,
            ty: AstType::List(Box::new(first_type)),
            span,
        })
    }

    fn check_select(
        &self,
        clauses: &[SelectClause],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        if clauses.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "non-empty select".to_string(),
                found: "empty select".to_string(),
                span,
                file_id: ctx.file_id,
            });
        }

        let first = &clauses[0];
        let typed_first_run = self.check_expression(&first.expression_to_run, env, ctx)?;
        let first_result_type = typed_first_run.ty().clone();
        let mut first_env = env.create_child();
        first_env.declare_variable(
            first.result_variable.clone(),
            first_result_type,
            first.expression_to_run.span(),
        );
        let typed_first_next = self.check_expression(&first.expression_next, &first_env, ctx)?;
        let first_type = typed_first_next.ty().clone();

        let mut typed_clauses = vec![typed_ast::SelectClause {
            expression_to_run: typed_first_run,
            result_variable: first.result_variable.clone(),
            expression_next: typed_first_next,
            span: first.span,
        }];

        for (i, clause) in clauses.iter().enumerate().skip(1) {
            let typed_run = self.check_expression(&clause.expression_to_run, env, ctx)?;
            let result_type = typed_run.ty().clone();
            let mut clause_env = env.create_child();
            clause_env.declare_variable(
                clause.result_variable.clone(),
                result_type,
                clause.expression_to_run.span(),
            );
            let typed_next = self.check_expression(&clause.expression_next, &clause_env, ctx)?;
            if first_type != *typed_next.ty() {
                return Err(TypeError::SelectBranchTypeMismatch {
                    expected: format!("{}", first_type),
                    found: format!("{}", typed_next.ty()),
                    branch_index: i,
                    span: clause.expression_next.span(),
                    first_branch_span: first.expression_next.span(),
                    file_id: ctx.file_id,
                });
            }
            typed_clauses.push(typed_ast::SelectClause {
                expression_to_run: typed_run,
                result_variable: clause.result_variable.clone(),
                expression_next: typed_next,
                span: clause.span,
            });
        }

        Ok(typed_ast::Expression::Select(
            typed_ast::SelectExpression {
                clauses: typed_clauses,
                span,
            },
            first_type,
        ))
    }

    fn check_if_else_expression(
        &self,
        condition: &Expression,
        then_expr: &Expression,
        else_expr: &Expression,
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let typed_condition = self.check_boolean_condition(condition, env, ctx)?;
        let typed_then = self.check_expression(then_expr, env, ctx)?;
        let typed_else = self.check_expression(else_expr, env, ctx)?;

        if typed_then.ty() != typed_else.ty() {
            return Err(TypeError::TypeMismatch {
                expected: format!("{}", typed_then.ty()),
                found: format!("{}", typed_else.ty()),
                span: else_expr.span(),
                file_id: ctx.file_id,
            });
        }

        let ty = typed_then.ty().clone();
        Ok(typed_ast::Expression::IfElse {
            condition: Box::new(typed_condition),
            then_expr: Box::new(typed_then),
            else_expr: Box::new(typed_else),
            ty,
            span,
        })
    }

    fn check_struct_literal(
        &self,
        struct_name: &str,
        fields: &[(String, Expression)],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let definition = self
            .struct_definitions
            .get(struct_name)
            .ok_or_else(|| TypeError::UnsupportedType {
                type_name: struct_name.to_string(),
                span,
                file_id: ctx.file_id,
            })?
            .clone();

        let mut seen = std::collections::HashSet::new();
        let mut typed_fields = Vec::new();

        for (field_name, value_expr) in fields {
            if !seen.insert(field_name.clone()) {
                return Err(TypeError::DuplicateField {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                });
            }

            let declared_type = definition
                .iter()
                .find(|(n, _)| n == field_name)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| TypeError::UnknownField {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                })?;

            let typed_value = self.check_expression(value_expr, env, ctx)?;
            if typed_value.ty() != &declared_type {
                return Err(TypeError::StructFieldTypeMismatch {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    expected: format!("{}", declared_type),
                    found: format!("{}", typed_value.ty()),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                });
            }
            typed_fields.push((field_name.clone(), typed_value));
        }

        for (required_field, _) in &definition {
            if !fields.iter().any(|(n, _)| n == required_field) {
                return Err(TypeError::MissingField {
                    struct_name: struct_name.to_string(),
                    field_name: required_field.clone(),
                    span,
                    file_id: ctx.file_id,
                });
            }
        }

        Ok(typed_ast::Expression::StructLiteral {
            struct_name: struct_name.to_string(),
            fields: typed_fields,
            ty: AstType::Struct(struct_name.to_string()),
            span,
        })
    }

    fn check_field_access(
        &self,
        base: &Expression,
        field: &str,
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let typed_base = self.check_expression(base, env, ctx)?;
        let base_type = typed_base.ty().clone();
        match base_type {
            AstType::Struct(name) | AstType::Generic(name) => {
                let definition = self.struct_definitions.get(&name).ok_or_else(|| {
                    TypeError::UnsupportedType {
                        type_name: name.clone(),
                        span,
                        file_id: ctx.file_id,
                    }
                })?;
                let field_type = definition
                    .iter()
                    .find(|(n, _)| n == field)
                    .map(|(_, t)| t.clone())
                    .ok_or_else(|| TypeError::UnknownField {
                        struct_name: name.clone(),
                        field_name: field.to_string(),
                        span,
                        file_id: ctx.file_id,
                    })?;
                Ok(typed_ast::Expression::FieldAccess {
                    base: Box::new(typed_base),
                    field: field.to_string(),
                    ty: field_type,
                    span,
                })
            }
            other => Err(TypeError::TypeMismatch {
                expected: "struct".to_string(),
                found: format!("{}", other),
                span,
                file_id: ctx.file_id,
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExternalSig {
    pub parameters: Vec<Parameter>,
    pub return_type: AstType,
    pub is_pub: bool,
    pub kind: FunctionKind,
    pub type_params: Vec<String>,
}

impl ExternalSig {
    pub fn new(
        parameters: Vec<crate::ast::Parameter>,
        return_type: crate::ast::Type,
        is_pub: bool,
        kind: FunctionKind,
    ) -> Self {
        Self {
            parameters,
            return_type,
            is_pub,
            kind,
            type_params: vec![],
        }
    }

    pub fn with_type_params(mut self, type_params: Vec<String>) -> Self {
        self.type_params = type_params;
        self
    }
}

impl TypeChecker {
    pub fn function_kinds(&self) -> HashMap<String, FunctionKind> {
        self.function_signatures
            .iter()
            .map(|(name, sig)| (name.clone(), sig.kind.clone()))
            .collect()
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

    fn declare_variable(&mut self, name: String, var_type: AstType, span: Span) {
        self.variables.insert(name, (var_type, span));
    }

    fn lookup_variable(&self, name: &str) -> Option<AstType> {
        if let Some((ty, _)) = self.variables.get(name) {
            Some(ty.clone())
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable(name)
        } else {
            None
        }
    }

    fn lookup_variable_with_span(&self, name: &str) -> Option<(AstType, Span)> {
        if let Some((ty, span)) = self.variables.get(name) {
            Some((ty.clone(), *span))
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable_with_span(name)
        } else {
            None
        }
    }
}
