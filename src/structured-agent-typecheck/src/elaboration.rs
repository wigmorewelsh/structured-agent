use crate::db::build_module_instantiation;

use super::db::{
    Intern, InternedFunctionName, InternedModuleName, InternedTypeName, TypeCheckDatabase,
    find_impl_fn, get_function_sig, get_struct_fields, impl_for_type_and_trait,
    lookup_type_def_in_symbol_tables, resolve_function_call, resolve_type_in_module,
};
use super::synthesize;
use crate::FunctionSignature;
use structured_agent_ast::ast::{Expression, Function, Statement, Type as AstType};
use structured_agent_ast::types::{Span, Spanned};
use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::{DefinitionPath, FunctionKind, TypeDefinitionKind};
use structured_agent_typed_ast as typed_ast;

struct ResolvedCallee {
    sig: FunctionSignature,
    binding: typed_ast::MethodBinding,
    receiver_is_target: bool,
}

fn implicit_param_name(type_param: &str, trait_name: &str) -> String {
    format!("__{}__{}", type_param, trait_name)
}

pub fn elaborate_function(
    db: &dyn TypeCheckDatabase,
    func: &Function,
    ctx: &synthesize::CheckContext,
    self_type: Option<DefinitionPath>,
) -> Option<typed_ast::Function> {
    let mut env = synthesize::TypeEnvironment::with_type_params(&func.type_params);
    if let Some(st) = self_type {
        env.set_self_type(st);
    }
    let mut typed_parameters = Vec::new();
    for tp in &func.type_params {
        let rt_type = RT::Generic(tp.name.clone());
        let binding_id = env.declare_variable(tp.name.clone(), rt_type.clone(), func.span);
        typed_parameters.push(typed_ast::Parameter {
            name: tp.name.clone(),
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
        Statement::Yield { span } => Some((typed_ast::Statement::Yield { span: *span }, env)),
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
            type_args,
            arguments,
            span,
        } => elaborate_call(db, function, type_args, arguments, *span, env, ctx),
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
        Expression::Spawn {
            type_arg,
            key,
            span,
        } => elaborate_spawn(db, type_arg, key, *span, env, ctx),
    }
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
    let callee = resolve_method_callee(db, typed_receiver.ty(), method, span, env, ctx)?;
    build_typed_call(
        db,
        &callee,
        method.to_string(),
        &[],
        Some(typed_receiver),
        args,
        span,
        env,
        ctx,
    )
}

fn resolve_method_callee(
    db: &dyn TypeCheckDatabase,
    receiver_type: &RT,
    method: &str,
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<ResolvedCallee> {
    if receiver_type.is_actor_ref() {
        return resolve_callee_for_actor_method(db, receiver_type, method, ctx);
    }
    if let RT::Generic(param_name) = receiver_type {
        return resolve_callee_for_generic_method(db, env, param_name, method, span, ctx);
    }
    let struct_type_name = match receiver_type {
        RT::Named(tn) => tn.last_name().to_string(),
        _ => return None,
    };
    resolve_callee_for_method(db, &struct_type_name, method, env, ctx)
}

fn elaborate_spawn(
    db: &dyn TypeCheckDatabase,
    type_arg: &AstType,
    key: &Expression,
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let current_module = InternedModuleName::new(db, ctx.module_name.clone());
    let module_path = build_module_instantiation(db, current_module, &type_arg.path)?;
    let typed_key = elaborate_expression(db, key, env, ctx)?;
    let actor_ref_type = RT::actor_ref(RT::Named(module_path));
    Some(typed_ast::Expression::Spawn {
        key: Box::new(typed_key),
        ty: actor_ref_type,
        span,
    })
}

fn resolve_callee_for_call(
    db: &dyn TypeCheckDatabase,
    function: &str,
    _env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<ResolvedCallee> {
    let interned_current = InternedModuleName::new(db, ctx.module_name.clone());
    let interned_fn = function.intern(db);
    let (resolved_fn_name, sig) = resolve_function_call(db, interned_current, interned_fn)
        .and_then(|fn_name| {
            let interned = InternedFunctionName::new(db, fn_name.clone());
            get_function_sig(db, interned, ctx.program).map(|arc| (fn_name, arc.get().clone()))
        })?;
    Some(ResolvedCallee {
        sig,
        binding: typed_ast::MethodBinding::Early(resolved_fn_name),
        receiver_is_target: false,
    })
}

fn resolve_callee_for_method(
    db: &dyn TypeCheckDatabase,
    struct_type_name: &str,
    method: &str,
    _env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<ResolvedCallee> {
    let impl_fn_path = find_impl_fn(db, struct_type_name, method, ctx.module_name)?;
    let sig = get_function_sig(
        db,
        InternedFunctionName::new(db, impl_fn_path.clone()),
        ctx.program,
    )?
    .get()
    .clone();
    Some(ResolvedCallee {
        sig,
        binding: typed_ast::MethodBinding::Early(impl_fn_path),
        receiver_is_target: false,
    })
}

fn resolve_callee_for_actor_method(
    db: &dyn TypeCheckDatabase,
    receiver_type: &RT,
    method: &str,
    ctx: &synthesize::CheckContext,
) -> Option<ResolvedCallee> {
    let module_type = receiver_type.actor_ref_inner()?;
    let module_path = match module_type {
        RT::Named(p) | RT::Parameterized(p, _) => p.clone(),
        _ => return None,
    };
    let fn_path = DefinitionPath::for_function(module_path.clone(), method);
    let sig = get_function_sig(
        db,
        InternedFunctionName::new(db, fn_path.clone()),
        ctx.program,
    )?
    .get()
    .clone();
    Some(ResolvedCallee {
        sig,
        binding: typed_ast::MethodBinding::Early(fn_path),
        receiver_is_target: true,
    })
}

fn resolve_callee_for_generic_method(
    db: &dyn TypeCheckDatabase,
    env: &synthesize::TypeEnvironment,
    param_name: &str,
    method: &str,
    span: Span,
    ctx: &synthesize::CheckContext,
) -> Option<ResolvedCallee> {
    let (sig, trait_path, bound_short_name) =
        synthesize::resolve_generic_method_sig(db, param_name, method, env, span, ctx)?;
    let implicit_name = implicit_param_name(param_name, &bound_short_name);
    let (_, binding_id) = env.lookup_variable(&implicit_name)?;
    let impl_fn_path = DefinitionPath::for_impl_fn(&trait_path, method);
    Some(ResolvedCallee {
        sig,
        binding: typed_ast::MethodBinding::Late(binding_id, impl_fn_path),
        receiver_is_target: false,
    })
}

fn elaborate_user_args(
    db: &dyn TypeCheckDatabase,
    sig: &FunctionSignature,
    unifier: &mut synthesize::Unifier,
    param_offset: usize,
    pending_args: &[Expression],
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<Vec<typed_ast::Expression>> {
    let mut user_args = Vec::new();
    for (arg, param) in pending_args
        .iter()
        .zip(sig.parameters.iter().skip(param_offset))
    {
        if matches!(arg, Expression::Placeholder { .. }) {
            user_args.push(typed_ast::Expression::Placeholder {
                ty: param.param_type.clone(),
                span: arg.span(),
            });
            continue;
        }
        let user_arg = elaborate_expression(db, arg, env, ctx)?;
        let _ = unifier.unify_type(&param.param_type, user_arg.ty());
        user_args.push(user_arg);
    }
    Some(user_args)
}

fn elaborate_type_args(
    db: &dyn TypeCheckDatabase,
    sig: &FunctionSignature,
    unifier: &synthesize::Unifier,
    type_args: &[AstType],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Vec<typed_ast::Expression> {
    let mut type_arg_exprs: Vec<typed_ast::Expression> = Vec::new();
    for tp in sig
        .type_params
        .iter()
        .filter(|_| sig.kind == FunctionKind::Bytecode)
    {
        let resolved = if let Some(idx) = sig.type_params.iter().position(|p| p.name == tp.name)
            && let Some(explicit) = type_args.get(idx)
        {
            synthesize::resolve(db, explicit, env, span, ctx)
        } else {
            unifier
                .get(&tp.name)
                .cloned()
                .or_else(|| Some(RT::Generic(tp.name.clone())))
        };
        let Some(ty) = resolved else { continue };
        let expr = match &ty {
            RT::Generic(name) => {
                if let Some((var_ty, binding_id)) = env.lookup_variable(name) {
                    typed_ast::Expression::Variable {
                        name: name.clone(),
                        binding_id,
                        ty: var_ty,
                        span,
                    }
                } else {
                    typed_ast::Expression::TypeLiteral { ty, span }
                }
            }
            _ => typed_ast::Expression::TypeLiteral { ty, span },
        };
        type_arg_exprs.push(expr);
    }
    type_arg_exprs
}

fn elaborate_implicit_trait_args(
    db: &dyn TypeCheckDatabase,
    sig: &FunctionSignature,
    unifier: &synthesize::Unifier,
    span: Span,
    ctx: &synthesize::CheckContext,
) -> Vec<typed_ast::Expression> {
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
    implicit_args
}

fn elaborate_arguments(
    db: &dyn TypeCheckDatabase,
    sig: &FunctionSignature,
    unifier: &mut synthesize::Unifier,
    type_args: &[AstType],
    pending_args: &[Expression],
    elaborated_receiver: Option<typed_ast::Expression>,
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<Vec<typed_ast::Expression>> {
    let param_offset = usize::from(elaborated_receiver.is_some());
    let user_args = elaborate_user_args(db, sig, unifier, param_offset, pending_args, env, ctx)?;
    let type_exprs = elaborate_type_args(db, sig, unifier, type_args, span, env, ctx);
    let trait_exprs = elaborate_implicit_trait_args(db, sig, unifier, span, ctx);
    let mut all_args = type_exprs;
    all_args.extend(trait_exprs);
    if let Some(receiver) = elaborated_receiver {
        all_args.push(receiver);
    }
    all_args.extend(user_args);
    Some(all_args)
}

fn build_typed_call(
    db: &dyn TypeCheckDatabase,
    callee: &ResolvedCallee,
    function_name: String,
    type_args: &[AstType],
    receiver: Option<typed_ast::Expression>,
    pending_args: &[Expression],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let sig = &callee.sig;
    let mut unifier = synthesize::Unifier::new();
    for (tp, ty_arg) in sig.type_params.iter().zip(type_args) {
        if let Some(resolved) = synthesize::resolve(db, ty_arg, env, span, ctx) {
            let _ = unifier.unify_type(&RT::Generic(tp.name.clone()), &resolved);
        }
    }
    let (elaborated_receiver, target) = if callee.receiver_is_target {
        let t = receiver.map(Box::new);
        (None, t)
    } else {
        (receiver, None)
    };
    if let Some(r) = &elaborated_receiver
        && let Some(param) = sig.parameters.first()
    {
        let _ = unifier.unify_type(&param.param_type, r.ty());
    }
    let all_args = elaborate_arguments(
        db,
        sig,
        &mut unifier,
        type_args,
        pending_args,
        elaborated_receiver,
        span,
        env,
        ctx,
    )?;
    let resolved_return = unifier.apply_subst(&sig.return_type);
    Some(typed_ast::Expression::Call {
        function: function_name,
        binding: callee.binding.clone(),
        kind: sig.kind.clone(),
        arguments: all_args,
        target,
        ty: resolved_return,
        span,
    })
}

fn elaborate_call(
    db: &dyn TypeCheckDatabase,
    function: &str,
    type_args: &[AstType],
    arguments: &[Expression],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let callee = resolve_callee_for_call(db, function, env, ctx)?;
    build_typed_call(
        db,
        &callee,
        function.to_string(),
        type_args,
        None,
        arguments,
        span,
        env,
        ctx,
    )
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
