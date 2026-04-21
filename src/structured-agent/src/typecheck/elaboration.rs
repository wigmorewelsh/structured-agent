use super::db::{
    Intern, SymbolTablesInput, TypeCheckDatabase, get_function_sig, get_struct_fields,
    resolve_function_alias_via_param, resolve_function_call, resolve_type_in_module,
    resolve_use_param_bindings,
};
use super::synthesize;
use crate::ast::{Expression, Function, SelectClause, Statement};
use crate::typed_ast;
use crate::types::{Span, Spanned};

use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::TypeName;

pub(super) fn elaborate_function(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    func: &Function,
    ctx: &synthesize::CheckContext,
    self_type: Option<structured_agent_runtime::symbols::TypeName>,
) -> Option<typed_ast::Function> {
    let mut env = synthesize::TypeEnvironment::with_type_params(&func.type_params);
    if let Some(st) = self_type {
        env.set_self_type(st);
    }
    let mut typed_parameters = Vec::new();
    for param in &func.parameters {
        let runtime_type =
            synthesize::resolve(db, tables, &param.param_type, &env, param.span, ctx)?;
        env.declare_variable(param.name.clone(), runtime_type.clone(), param.span);
        typed_parameters.push(typed_ast::Parameter {
            name: param.name.clone(),
            param_type: runtime_type,
            span: param.span,
        });
    }
    let runtime_return_type =
        synthesize::resolve(db, tables, &func.return_type, &env, func.span, ctx)?;
    let typed_stmts = elaborate_block(db, tables, &func.body.statements, env, ctx)?;
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
    tables: SymbolTablesInput,
    statement: &Statement,
    mut env: synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<(typed_ast::Statement, synthesize::TypeEnvironment)> {
    match statement {
        Statement::Injection(expr) => {
            let typed_expr = elaborate_expression(db, tables, expr, &env, ctx)?;
            Some((typed_ast::Statement::Injection(typed_expr), env))
        }
        Statement::Assignment {
            variable,
            expression,
            span,
        } => {
            let typed_expr = elaborate_expression(db, tables, expression, &env, ctx)?;
            let expr_type = typed_expr.ty().clone();
            env.declare_variable(variable.clone(), expr_type, expression.span());
            Some((
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
            let typed_expr = elaborate_expression(db, tables, expression, &env, ctx)?;
            Some((
                typed_ast::Statement::VariableAssignment {
                    variable: variable.clone(),
                    expression: typed_expr,
                    span: *span,
                },
                env,
            ))
        }
        Statement::ExpressionStatement(expr) => {
            let typed_expr = elaborate_expression(db, tables, expr, &env, ctx)?;
            Some((typed_ast::Statement::ExpressionStatement(typed_expr), env))
        }
        Statement::If {
            condition,
            body,
            else_body,
            span,
        } => {
            let typed_condition = elaborate_expression(db, tables, condition, &env, ctx)?;
            let typed_body = elaborate_block(db, tables, body, env.create_child(), ctx)?;
            let typed_else = if let Some(else_stmts) = else_body {
                Some(elaborate_block(
                    db,
                    tables,
                    else_stmts,
                    env.create_child(),
                    ctx,
                )?)
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
            let typed_condition = elaborate_expression(db, tables, condition, &env, ctx)?;
            let typed_body = elaborate_block(db, tables, body, env.create_child(), ctx)?;
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
            let typed_expr = elaborate_expression(db, tables, expr, &env, ctx)?;
            Some((typed_ast::Statement::Return(typed_expr), env))
        }
    }
}

fn elaborate_block(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    statements: &[Statement],
    mut env: synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<Vec<typed_ast::Statement>> {
    let mut typed_stmts = Vec::new();
    for stmt in statements {
        let (typed_stmt, new_env) = elaborate_statement(db, tables, stmt, env, ctx)?;
        typed_stmts.push(typed_stmt);
        env = new_env;
    }
    Some(typed_stmts)
}

pub(super) fn elaborate_expression(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    expression: &Expression,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    match expression {
        Expression::Call {
            function,
            arguments,
            span,
        } => elaborate_call(db, tables, function, arguments, *span, env, ctx),
        Expression::Variable { name, span } => {
            let ty = env.lookup_variable(name)?;
            Some(typed_ast::Expression::Variable {
                name: name.clone(),
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
            elaborate_list_literal(db, tables, elements, *span, env, ctx)
        }
        Expression::Select(select) => {
            elaborate_select(db, tables, &select.clauses, select.span, env, ctx)
        }
        Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            span,
        } => elaborate_if_else(db, tables, condition, then_expr, else_expr, *span, env, ctx),
        Expression::StructLiteral {
            struct_name,
            fields,
            span,
        } => elaborate_struct_literal(db, tables, struct_name, fields, *span, env, ctx),
        Expression::FieldAccess { base, field, span } => {
            elaborate_field_access(db, tables, base, field, *span, env, ctx)
        }
    }
}

fn elaborate_call(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    function: &str,
    arguments: &[Expression],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let interned_current = ctx.module_name.intern(db);
    let interned_fn = function.intern(db);

    let (resolved_fn_name, sig) = resolve_function_call(db, tables, interned_current, interned_fn)
        .and_then(|fn_name| {
            let interned = fn_name.clone().intern(db);
            get_function_sig(db, tables, interned, ctx.program)
                .map(|arc| (fn_name, arc.get().clone()))
        })?;

    let solved_constraints = crate::typecheck::solver::solve_constraints(db, ctx.program, tables);
    let type_arguments = solved_constraints
        .resolved
        .get(&ctx.module_name.to_string())
        .and_then(|callees| callees.get(function))
        .cloned()
        .unwrap_or_default();

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
        let typed_arg = elaborate_expression(db, tables, arg, env, ctx)?;
        let _ = unifier.unify_type(&param.param_type, typed_arg.ty());
        typed_args.push(typed_arg);
    }
    let resolved_return = unifier.apply_subst(&sig.return_type);
    let module_params = resolve_use_param_bindings(db, tables, interned_current, interned_fn);
    let via_module_param = resolve_function_alias_via_param(db, tables, interned_current, interned_fn);
    Some(typed_ast::Expression::Call {
        function: function.to_string(),
        resolved: resolved_fn_name,
        kind: sig.kind,
        type_arguments,
        arguments: typed_args,
        module_params,
        via_module_param,
        ty: resolved_return,
        span,
    })
}

fn elaborate_list_literal(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    elements: &[Expression],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    if elements.is_empty() {
        return None;
    }
    let typed_first = elaborate_expression(db, tables, &elements[0], env, ctx)?;
    let first_type = typed_first.ty().clone();
    let mut typed_elements = vec![typed_first];
    for elem in elements.iter().skip(1) {
        let typed_elem = elaborate_expression(db, tables, elem, env, ctx)?;
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
    tables: SymbolTablesInput,
    clauses: &[SelectClause],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    if clauses.is_empty() {
        return None;
    }
    let first = &clauses[0];
    let typed_first_run = elaborate_expression(db, tables, &first.expression_to_run, env, ctx)?;
    let mut first_env = env.create_child();
    first_env.declare_variable(
        first.result_variable.clone(),
        typed_first_run.ty().clone(),
        first.expression_to_run.span(),
    );
    let typed_first_next =
        elaborate_expression(db, tables, &first.expression_next, &first_env, ctx)?;
    let first_type = typed_first_next.ty().clone();
    let mut typed_clauses = vec![typed_ast::SelectClause {
        expression_to_run: typed_first_run,
        result_variable: first.result_variable.clone(),
        expression_next: typed_first_next,
        span: first.span,
    }];
    for clause in clauses.iter().skip(1) {
        let typed_run = elaborate_expression(db, tables, &clause.expression_to_run, env, ctx)?;
        let mut clause_env = env.create_child();
        clause_env.declare_variable(
            clause.result_variable.clone(),
            typed_run.ty().clone(),
            clause.expression_to_run.span(),
        );
        let typed_next =
            elaborate_expression(db, tables, &clause.expression_next, &clause_env, ctx)?;
        typed_clauses.push(typed_ast::SelectClause {
            expression_to_run: typed_run,
            result_variable: clause.result_variable.clone(),
            expression_next: typed_next,
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
    tables: SymbolTablesInput,
    condition: &Expression,
    then_expr: &Expression,
    else_expr: &Expression,
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let typed_condition = elaborate_expression(db, tables, condition, env, ctx)?;
    let typed_then = elaborate_expression(db, tables, then_expr, env, ctx)?;
    let typed_else = elaborate_expression(db, tables, else_expr, env, ctx)?;
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
    tables: SymbolTablesInput,
    struct_name: &str,
    fields: &[(String, Expression)],
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let (definition, type_params) = get_struct_fields(db, tables, struct_name, ctx.module_name)?;
    let type_env = synthesize::TypeEnvironment::with_type_params(&type_params);
    let mut typed_fields = Vec::new();
    let mut unifier = synthesize::Unifier::new();
    for (field_name, value_expr) in fields {
        let declared_ast_type = definition
            .iter()
            .find(|(n, _)| n == field_name)
            .map(|(_, t)| t.clone())?;
        let declared_type = synthesize::resolve(
            db,
            tables,
            &declared_ast_type,
            &type_env,
            value_expr.span(),
            ctx,
        )?;
        let typed_value = elaborate_expression(db, tables, value_expr, env, ctx)?;
        let _ = unifier.unify_type(&declared_type, typed_value.ty());
        typed_fields.push((field_name.clone(), typed_value));
    }
    let resolved_type_name = {
        let interned_mod = ctx.module_name.intern(db);
        let interned_name = struct_name.intern(db);
        resolve_type_in_module(db, tables, interned_mod, interned_name)
            .unwrap_or_else(|| TypeName::new(ctx.module_name.clone(), struct_name))
    };
    let ty = if type_params.is_empty() {
        RT::Struct(resolved_type_name)
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
    tables: SymbolTablesInput,
    base: &Expression,
    field: &str,
    span: Span,
    env: &synthesize::TypeEnvironment,
    ctx: &synthesize::CheckContext,
) -> Option<typed_ast::Expression> {
    let typed_base = elaborate_expression(db, tables, base, env, ctx)?;
    let base_type = typed_base.ty().clone();
    let struct_type_name = match &base_type {
        RT::Struct(tn) => tn.name().to_string(),
        RT::Generic(name) => name.clone(),
        _ => return None,
    };
    let (definition, _) = get_struct_fields(db, tables, &struct_type_name, ctx.module_name)?;
    let field_ast_type = definition
        .iter()
        .find(|(n, _)| n == field)
        .map(|(_, t)| t.clone())?;
    let empty_env = synthesize::TypeEnvironment::new();
    let field_ty = synthesize::resolve(db, tables, &field_ast_type, &empty_env, span, ctx)?;
    Some(typed_ast::Expression::FieldAccess {
        base: Box::new(typed_base),
        field: field.to_string(),
        ty: field_ty,
        span,
    })
}
