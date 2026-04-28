use super::db::{
    CallModuleArg, Intern, InternedFunctionName, InternedModuleName, InternedTypeName,
    ModuleInstantiation, TypeCheckDatabase, find_impl_fn, get_function_sig, get_struct_fields,
    impl_for_type_and_trait, lookup_type_def_in_symbol_tables, resolve_call_routing,
    resolve_function_call, resolve_type_in_module,
};
use super::synthesize;
use structured_agent_ast::ast::{Expression, Function, Statement};
use structured_agent_ast::types::{Span, Spanned};
use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::{DefinitionPath, FunctionKind, TypeDefinitionKind};
use structured_agent_typed_ast as typed_ast;

fn implicit_param_name(type_param: &str, trait_name: &str) -> String {
    format!("__{}__{}", type_param, trait_name)
}

fn find_implicit_param(
    env: &synthesize::TypeEnvironment,
    prefix: &str,
) -> Option<(RT, typed_ast::BindingId)> {
    for (name, (ty, id, _)) in &env.variables {
        if name.starts_with(prefix) {
            return Some((ty.clone(), *id));
        }
    }
    if let Some(parent) = &env.parent {
        find_implicit_param(parent, prefix)
    } else {
        None
    }
}

pub fn elaborate_function(
    db: &dyn TypeCheckDatabase,
    func: &Function,
    ctx: &synthesize::CheckContext,
    self_type: Option<DefinitionPath>,
    module_params: &[(String, DefinitionPath)],
) -> Option<typed_ast::Function> {
    let mut env = synthesize::TypeEnvironment::with_type_params(&func.type_params);
    if let Some(st) = self_type {
        env.set_self_type(st);
    }
    let mut typed_parameters = Vec::new();
    for (name, module_type_path) in module_params {
        let rt_type = structured_agent_runtime::Type::Named(module_type_path.clone());
        let binding_id = env.declare_variable(name.clone(), rt_type.clone(), func.span);
        typed_parameters.push(typed_ast::Parameter {
            name: name.clone(),
            param_type: rt_type,
            binding_id,
            span: func.span,
        });
    }
    let interned_mod = InternedModuleName::new(db, ctx.module_name.clone());
    for tp in &func.type_params {
        for bound in &tp.bounds {
            let bound_interned = bound.name().to_string().intern(db);
            let Some(resolved) = resolve_type_in_module(db, interned_mod, bound_interned) else {
                continue;
            };
            let trait_path = resolved.ty;
            let Some(type_def) =
                lookup_type_def_in_symbol_tables(db, InternedTypeName::new(db, trait_path.clone()))
            else {
                continue;
            };
            if !matches!(type_def.get().kind, TypeDefinitionKind::Trait { .. }) {
                continue;
            }
            let param_name = implicit_param_name(&tp.name, bound.name());
            let rt_type = RT::Named(trait_path.clone());
            let binding_id = env.declare_variable(param_name.clone(), rt_type.clone(), func.span);
            typed_parameters.push(typed_ast::Parameter {
                name: param_name,
                param_type: rt_type,
                binding_id,
                span: func.span,
            });
        }
    }
    for param in &func.parameters {
        let runtime_type = synthesize::resolve(db, &param.param_type, &env, param.span, ctx)?;
        let binding_id = env.declare_variable(param.name.clone(), runtime_type.clone(), param.span);
        typed_parameters.push(typed_ast::Parameter {
            name: param.name.clone(),
            param_type: runtime_type,
            binding_id,
            span: param.span,
        });
    }
    let runtime_return_type = synthesize::resolve(db, &func.return_type, &env, func.span, ctx)?;
    let typed_stmts = elaborate_block(db, &func.body.statements, env, ctx)?;
    Some(typed_ast::Function {
        name: func.name.clone(),
        parameters: typed_parameters,
        return_type: runtime_return_type,
        body: typed_ast::FunctionBody {
            statements: typed_stmts,
            span: func.body.span,
        },
        documentation: func.documentation.clone(),
        is_pub: func.is_pub,
        span: func.span,
    })
}

fn elaborate_statement(
    db: &dyn TypeCheckDatabase,
    statement: &Statement,
    mut env: synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<(typed_ast::Statement, synthesize::TypeEnvironment)> {
    match statement {
        Statement::Injection(expr) => {
            let typed_expr = elaborate_expression(db, expr, &env, ctx)?;
            Some((typed_ast::Statement::Injection(typed_expr), env))
        }
        Statement::Assignment {
            variable,
            expression,
            span,
        } => {
            let typed_expr = elaborate_expression(db, expression, &env, ctx)?;
            let expr_type = typed_expr.ty().clone();
            let binding_id = env.declare_variable(variable.clone(), expr_type, expression.span());
            Some((
                typed_ast::Statement::Assignment {
                    variable: variable.clone(),
                    binding_id,
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
            let typed_expr = elaborate_expression(db, expression, &env, ctx)?;
            let binding_id = env.lookup_variable(variable)?.1;
            Some((
                typed_ast::Statement::VariableAssignment {
                    variable: variable.clone(),
                    binding_id,
                    expression: typed_expr,
                    span: *span,
                },
                env,
            ))
        }
        Statement::ExpressionStatement(expr) => {
            let typed_expr = elaborate_expression(db, expr, &env, ctx)?;
            Some((typed_ast::Statement::ExpressionStatement(typed_expr), env))
        }
        Statement::If {
            condition,
            body,
            else_body,
            span,
        } => {
            let typed_condition = elaborate_expression(db, condition, &env, ctx)?;
            let typed_body = elaborate_block(db, body, env.create_child(), ctx)?;
            let typed_else = if let Some(else_stmts) = else_body {
                Some(elaborate_block(db, else_stmts, env.create_child(), ctx)?)
            } else {
                None
            };
            Some((
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
            let typed_condition = elaborate_expression(db, condition, &env, ctx)?;
            let typed_body = elaborate_block(db, body, env.create_child(), ctx)?;
            Some((
                typed_ast::Statement::While {
                    condition: typed_condition,
                    body: typed_body,
                    span: *span,
                },
                env,
            ))
        }
        Statement::Return(expr) => {
            let typed_expr = elaborate_expression(db, expr, &env, ctx)?;
            Some((typed_ast::Statement::Return(typed_expr), env))
        }
    }
}

fn elaborate_block(
    db: &dyn TypeCheckDatabase,
    statements: &[Statement],
    mut env: synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<Vec<typed_ast::Statement>> {
    let mut typed_stmts = Vec::new();
    for stmt in statements {
        let (typed_stmt, new_env) = elaborate_statement(db, stmt, env, ctx)?;
        typed_stmts.push(typed_stmt);
        env = new_env;
    }
    Some(typed_stmts)
}

pub fn elaborate_expression(
    db: &dyn TypeCheckDatabase,
    expression: &Expression,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    match expression {
        Expression::Call {
            function,
            arguments,
            span,
        } => elaborate_call(db, function, arguments, *span, env, ctx),
        Expression::Variable { name, span } => {
            let (ty, binding_id) = env.lookup_variable(name)?;
            Some(typed_ast::Expression::Variable {
                name: name.clone(),
                binding_id,
                ty,
                span: *span,
            })
        }
        Expression::StringLiteral { value, span } => Some(typed_ast::Expression::StringLiteral {
            value: value.clone(),
            ty: RT::string(),
            span: *span,
        }),
        Expression::BooleanLiteral { value, span } => Some(typed_ast::Expression::BooleanLiteral {
            value: *value,
            ty: RT::boolean(),
            span: *span,
        }),
        Expression::IntLiteral { value, span } => Some(typed_ast::Expression::IntLiteral {
            value: *value,
            ty: RT::int(),
            span: *span,
        }),
        Expression::UnitLiteral { span } => Some(typed_ast::Expression::UnitLiteral {
            ty: RT::unit(),
            span: *span,
        }),
        Expression::Placeholder { .. } => None,
        Expression::ListLiteral { elements, span } => {
            elaborate_list_literal(db, elements, *span, env, ctx)
        }
        Expression::Select(select) => elaborate_select(db, &select.clauses, select.span, env, ctx),
        Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            span,
        } => elaborate_if_else(db, condition, then_expr, else_expr, *span, env, ctx),
        Expression::StructLiteral {
            struct_name,
            fields,
            span,
        } => elaborate_struct_literal(db, struct_name, fields, *span, env, ctx),
        Expression::FieldAccess { base, field, span } => {
            elaborate_field_access(db, base, field, *span, env, ctx)
        }
        Expression::MethodCall {
            receiver,
            method,
            args,
            span,
        } => elaborate_method_call(db, receiver, method, args, *span, env, ctx),
    }
}

fn trait_method_return_type(
    db: &dyn TypeCheckDatabase,
    trait_path: &DefinitionPath,
    method: &str,
    param_name: &str,
    env: &synthesize::TypeEnvironment,
    span: Span,
    ctx: &synthesize::CheckContext,
) -> RT {
    let types_arcptr = db.symbol_tables().types(db);
    let types_map = types_arcptr.get();
    if let Some(trait_def) = types_map.get(trait_path)
        && let TypeDefinitionKind::Trait { functions, .. } = &trait_def.kind
        && let Some(entry) = functions.iter().find(|e| e.name == method)
        && let Some(fn_type_def) = types_map.get(&entry.type_name)
        && let TypeDefinitionKind::Function { return_type, .. } = &fn_type_def.kind
    {
        if return_type.name() == "Self" {
            return RT::Generic(param_name.to_string());
        }
        if let Some(rt) = synthesize::resolve(db, return_type, env, span, ctx) {
            return rt;
        }
    }
    RT::Generic(param_name.to_string())
}

fn elaborate_method_call(
    db: &dyn TypeCheckDatabase,
    receiver: &Expression,
    method: &str,
    args: &[Expression],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let typed_receiver = elaborate_expression(db, receiver, env, ctx)?;
    let receiver_type = typed_receiver.ty().clone();

    if let RT::Generic(param_name) = &receiver_type {
        let prefix = format!("__{}__", param_name);
        if let Some((trait_type, binding_id)) = find_implicit_param(env, &prefix)
            && let RT::Named(trait_path) = trait_type
        {
            let impl_fn_path = DefinitionPath::for_impl_fn(&trait_path, method);
            let mut typed_args = vec![typed_receiver];
            for arg in args {
                typed_args.push(elaborate_expression(db, arg, env, ctx)?);
            }
            let return_ty =
                trait_method_return_type(db, &trait_path, method, param_name, env, span, ctx);
            return Some(typed_ast::Expression::Call {
                function: method.to_string(),
                binding: typed_ast::MethodBinding::Late(binding_id, impl_fn_path),
                kind: FunctionKind::Bytecode,
                arguments: typed_args,
                ty: return_ty,
                span,
            });
        }
        return None;
    }

    let struct_type_name = match &receiver_type {
        RT::Named(tn) => tn.last_name().to_string(),
        _ => return None,
    };
    let impl_fn_path = find_impl_fn(db, &struct_type_name, method, ctx.module_name)?;
    let sig = get_function_sig(
        db,
        InternedFunctionName::new(db, impl_fn_path.clone()),
        ctx.program,
    )?
    .get()
    .clone();
    let mut typed_args = vec![typed_receiver];
    for arg in args {
        typed_args.push(elaborate_expression(db, arg, env, ctx)?);
    }
    Some(typed_ast::Expression::Call {
        function: method.to_string(),
        binding: typed_ast::MethodBinding::Early(impl_fn_path),
        kind: sig.kind,
        arguments: typed_args,
        ty: sig.return_type,
        span,
    })
}

fn elaborate_call(
    db: &dyn TypeCheckDatabase,
    function: &str,
    arguments: &[Expression],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let interned_current = InternedModuleName::new(db, ctx.module_name.clone());
    let interned_fn = function.intern(db);

    let (resolved_fn_name, sig) = resolve_function_call(db, interned_current, interned_fn)
        .and_then(|fn_name| {
            let interned = InternedFunctionName::new(db, fn_name.clone());
            get_function_sig(db, interned, ctx.program).map(|arc| (fn_name, arc.get().clone()))
        })?;

    let mut typed_args = Vec::new();

    let mut unifier = synthesize::Unifier::new();
    for (arg, param) in arguments.iter().zip(&sig.parameters) {
        if matches!(arg, Expression::Placeholder { .. }) {
            typed_args.push(typed_ast::Expression::Placeholder {
                ty: param.param_type.clone(),
                span: arg.span(),
            });
            continue;
        }
        let typed_arg = elaborate_expression(db, arg, env, ctx)?;
        let _ = unifier.unify_type(&param.param_type, typed_arg.ty());
        typed_args.push(typed_arg);
    }
    let resolved_return = unifier.apply_subst(&sig.return_type);
    let routing = resolve_call_routing(db, interned_current, interned_fn);

    let solved = crate::solver::solve_constraints(db, ctx.program);
    let mut implicit_args: Vec<typed_ast::Expression> = Vec::new();
    let interned_mod_call = InternedModuleName::new(db, ctx.module_name.clone());
    for tp in &sig.type_params {
        if tp.bounds.is_empty() {
            continue;
        }
        let Some(actual) = unifier.get(&tp.name) else {
            continue;
        };
        if matches!(actual, RT::Generic(_)) {
            continue;
        }
        let type_path = match actual {
            RT::Named(p) => p.clone(),
            RT::Parameterized(p, _) => p.clone(),
            _ => continue,
        };
        for bound in &tp.bounds {
            let bound_interned = bound.name().to_string().intern(db);
            let Some(resolved) = resolve_type_in_module(db, interned_mod_call, bound_interned)
            else {
                continue;
            };
            let trait_path = resolved.ty;
            let Some(type_def) =
                lookup_type_def_in_symbol_tables(db, InternedTypeName::new(db, trait_path.clone()))
            else {
                continue;
            };
            if !matches!(type_def.get().kind, TypeDefinitionKind::Trait { .. }) {
                continue;
            }
            let Some(impl_path) = impl_for_type_and_trait(&solved, &type_path, &trait_path) else {
                continue;
            };
            implicit_args.push(typed_ast::Expression::ModuleInstance {
                path: impl_path.clone(),
                params: vec![],
                ty: RT::Named(impl_path),
                span,
            });
        }
    }

    let binding = routing
        .as_ref()
        .and_then(|r| r.via_module_param.as_deref())
        .and_then(|name| env.lookup_variable(name).map(|(_, id)| id))
        .map(|id| typed_ast::MethodBinding::Late(id, resolved_fn_name.clone()))
        .unwrap_or_else(|| typed_ast::MethodBinding::Early(resolved_fn_name.clone()));

    let mut all_args: Vec<typed_ast::Expression> = implicit_args;

    if let Some(routing) = &routing {
        for arg in &routing.module_args {
            match arg {
                CallModuleArg::Concrete(instantiation) => {
                    all_args.push(instantiation_to_expr(instantiation, span));
                }
                CallModuleArg::FromParam(name) => {
                    if let Some((ty, binding_id)) = env.lookup_variable(name) {
                        all_args.push(typed_ast::Expression::Variable {
                            name: name.clone(),
                            binding_id,
                            ty,
                            span,
                        });
                    }
                }
            }
        }
    }

    all_args.extend(typed_args);

    Some(typed_ast::Expression::Call {
        function: function.to_string(),
        binding,
        kind: sig.kind,
        arguments: all_args,
        ty: resolved_return,
        span,
    })
}

fn instantiation_to_expr(
    inst: &ModuleInstantiation,
    span: structured_agent_ast::types::Span,
) -> typed_ast::Expression {
    typed_ast::Expression::ModuleInstance {
        path: inst.path.clone(),
        params: inst
            .params
            .iter()
            .map(|p| instantiation_to_expr(p, span))
            .collect(),
        ty: structured_agent_runtime::Type::Named(inst.path.clone()),
        span,
    }
}

fn elaborate_list_literal(
    db: &dyn TypeCheckDatabase,
    elements: &[Expression],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    if elements.is_empty() {
        return None;
    }
    let typed_first = elaborate_expression(db, &elements[0], env, ctx)?;
    let first_type = typed_first.ty().clone();
    let mut typed_elements = vec![typed_first];
    for elem in elements.iter().skip(1) {
        let typed_elem = elaborate_expression(db, elem, env, ctx)?;
        typed_elements.push(typed_elem);
    }
    Some(typed_ast::Expression::ListLiteral {
        elements: typed_elements,
        ty: RT::list(first_type),
        span,
    })
}

fn elaborate_select(
    db: &dyn TypeCheckDatabase,
    clauses: &[structured_agent_ast::ast::SelectClause],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    if clauses.is_empty() {
        return None;
    }
    let first = &clauses[0];
    let typed_first_run = elaborate_expression(db, &first.expression_to_run, env, ctx)?;
    let first_type = typed_first_run.ty().clone();
    let mut typed_clauses = vec![typed_ast::SelectClause {
        expression_to_run: typed_first_run,
        span: first.span,
    }];
    for clause in clauses.iter().skip(1) {
        let typed_run = elaborate_expression(db, &clause.expression_to_run, env, ctx)?;
        typed_clauses.push(typed_ast::SelectClause {
            expression_to_run: typed_run,
            span: clause.span,
        });
    }
    Some(typed_ast::Expression::Select(
        typed_ast::SelectExpression {
            clauses: typed_clauses,
            span,
        },
        first_type,
    ))
}

fn elaborate_if_else(
    db: &dyn TypeCheckDatabase,
    condition: &Expression,
    then_expr: &Expression,
    else_expr: &Expression,
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let typed_condition = elaborate_expression(db, condition, env, ctx)?;
    let typed_then = elaborate_expression(db, then_expr, env, ctx)?;
    let typed_else = elaborate_expression(db, else_expr, env, ctx)?;
    let ty = typed_then.ty().clone();
    Some(typed_ast::Expression::IfElse {
        condition: Box::new(typed_condition),
        then_expr: Box::new(typed_then),
        else_expr: Box::new(typed_else),
        ty,
        span,
    })
}

fn elaborate_struct_literal(
    db: &dyn TypeCheckDatabase,
    struct_name: &str,
    fields: &[(String, Expression)],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let (definition, type_params) = get_struct_fields(db, struct_name, ctx.module_name)?;
    let type_env = synthesize::TypeEnvironment::with_type_params(&type_params);
    let mut typed_fields = Vec::new();
    let mut unifier = synthesize::Unifier::new();
    for (field_name, value_expr) in fields {
        let declared_ast_type = definition
            .iter()
            .find(|(n, _)| n == field_name)
            .map(|(_, t)| t.clone())?;
        let declared_type =
            synthesize::resolve(db, &declared_ast_type, &type_env, value_expr.span(), ctx)?;
        let typed_value = elaborate_expression(db, value_expr, env, ctx)?;
        let _ = unifier.unify_type(&declared_type, typed_value.ty());
        typed_fields.push((field_name.clone(), typed_value));
    }
    let resolved_type_name = {
        let interned_mod = InternedModuleName::new(db, ctx.module_name.clone());
        let interned_name = struct_name.intern(db);
        resolve_type_in_module(db, interned_mod, interned_name)
            .map(|r| r.ty)
            .unwrap_or_else(|| DefinitionPath::for_type(ctx.module_name.clone(), struct_name))
    };
    let ty = if type_params.is_empty() {
        RT::Named(resolved_type_name)
    } else {
        let args: Vec<RT> = type_params
            .iter()
            .map(|tp| {
                unifier
                    .get(&tp.name)
                    .cloned()
                    .unwrap_or_else(|| RT::Generic(tp.name.clone()))
            })
            .collect();
        RT::Parameterized(resolved_type_name, args)
    };
    Some(typed_ast::Expression::StructLiteral {
        struct_name: struct_name.to_string(),
        fields: typed_fields,
        ty,
        span,
    })
}

fn elaborate_field_access(
    db: &dyn TypeCheckDatabase,
    base: &Expression,
    field: &str,
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let typed_base = elaborate_expression(db, base, env, ctx)?;
    let base_type = typed_base.ty().clone();
    let struct_type_name = match &base_type {
        RT::Named(tn) => tn.last_name().to_string(),
        RT::Generic(name) => name.clone(),
        _ => return None,
    };
    let (definition, _) = get_struct_fields(db, &struct_type_name, ctx.module_name)?;
    let field_ast_type = definition
        .iter()
        .find(|(n, _)| n == field)
        .map(|(_, t)| t.clone())?;
    let empty_env = synthesize::TypeEnvironment::new();
    let field_ty = synthesize::resolve(db, &field_ast_type, &empty_env, span, ctx)?;
    Some(typed_ast::Expression::FieldAccess {
        base: Box::new(typed_base),
        field: field.to_string(),
        ty: field_ty,
        span,
    })
}
