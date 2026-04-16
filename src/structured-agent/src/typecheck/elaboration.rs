use super::constraints::Unifier;
use super::db::{
    Intern, SymbolTablesInput, TypeCheckDatabase, get_function_sig, get_struct_fields,
    lookup_function_def, resolve_function_call, resolve_type_alias,
};
use super::error::OrAccumulateError;
use super::{CheckContext, TypeEnvironment};
use crate::ast::{Definition, Expression, Function, SelectClause, Statement};
use crate::ensure_or_accumulate;
use crate::typecheck::error::TypeError;
use crate::typed_ast;
use crate::types::{Span, Spanned};

use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::{FunctionName, FunctionNameKind, TypeName, Visibility};

pub(super) fn check_definition(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    definition: &Definition,
    ctx: &CheckContext,
) -> Option<()> {
    match definition {
        Definition::Function(func) => check_function(db, tables, func, ctx),
        Definition::ExternalFunction(f) => {
            let env = super::TypeEnvironment::with_type_params(&f.type_params);
            super::constraints::resolve(db, tables, &f.return_type, &env, f.span, ctx)?;
            for param in &f.parameters {
                super::constraints::resolve(db, tables, &param.param_type, &env, param.span, ctx)?;
            }
            Some(())
        }
        Definition::Struct(s) => {
            let env = super::TypeEnvironment::with_type_params(&s.type_params);
            for f in &s.fields {
                super::constraints::resolve(db, tables, &f.field_type, &env, f.span, ctx)?;
            }
            Some(())
        }
        Definition::Use { .. }
        | Definition::ModuleBinding { .. }
        | Definition::WiringSite { .. }
        | Definition::Trait(_) => Some(()),
        Definition::TraitImpl(_t) => Some(()),
        Definition::ModuleHeader { .. } | Definition::Signature(_) => unreachable!(),
    }
}

fn check_function(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    func: &Function,
    ctx: &CheckContext,
) -> Option<()> {
    let fn_name = FunctionName {
        name: func.name.clone(),
        module: ctx.module_name.clone(),
        kind: FunctionNameKind::Function,
    };
    let sig = get_function_sig(db, tables, fn_name.intern(db))?
        .get()
        .clone();
    let mut env = TypeEnvironment::new();
    for param in &sig.parameters {
        env.declare_variable(param.name.clone(), param.param_type.clone(), param.span);
    }
    check_block(
        db,
        tables,
        &func.body.statements,
        env,
        &func.name,
        &sig.return_type,
        ctx,
    )
}

fn check_statement(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    statement: &Statement,
    mut env: TypeEnvironment,
    function_name: &str,
    return_type: &RT,
    ctx: &CheckContext,
) -> Option<TypeEnvironment> {
    match statement {
        Statement::Injection(expr) => {
            synthesize_expression(db, tables, expr, &env, ctx)?;
            Some(env)
        }
        Statement::Assignment {
            variable,
            expression,
            span: _,
        } => {
            let ty = synthesize_expression(db, tables, expression, &env, ctx)?;
            env.declare_variable(variable.clone(), ty, expression.span());
            Some(env)
        }
        Statement::VariableAssignment {
            variable,
            expression,
            span,
        } => {
            let ty = synthesize_expression(db, tables, expression, &env, ctx)?;
            let (existing_type, declaration_span) =
                env.lookup_variable_with_span(variable).or_accumulate(
                    db,
                    TypeError::UnknownVariable {
                        name: variable.clone(),
                        span: *span,
                        file_id: ctx.file_id,
                    },
                )?;
            ensure_or_accumulate!(
                ty == existing_type,
                db,
                TypeError::VariableTypeMismatch {
                    variable: variable.clone(),
                    expected: existing_type.name(),
                    found: ty.name(),
                    span: expression.span(),
                    declaration_span,
                    file_id: ctx.file_id,
                }
            );
            Some(env)
        }
        Statement::ExpressionStatement(expr) => {
            synthesize_expression(db, tables, expr, &env, ctx)?;
            Some(env)
        }
        Statement::If {
            condition,
            body,
            else_body,
            span: _,
        } => {
            check_boolean_condition(db, tables, condition, &env, ctx)?;
            check_block(
                db,
                tables,
                body,
                env.create_child(),
                function_name,
                return_type,
                ctx,
            )?;
            if let Some(else_stmts) = else_body {
                check_block(
                    db,
                    tables,
                    else_stmts,
                    env.create_child(),
                    function_name,
                    return_type,
                    ctx,
                )?;
            }
            Some(env)
        }
        Statement::While {
            condition,
            body,
            span: _,
        } => {
            check_boolean_condition(db, tables, condition, &env, ctx)?;
            check_block(
                db,
                tables,
                body,
                env.create_child(),
                function_name,
                return_type,
                ctx,
            )?;
            Some(env)
        }
        Statement::Return(expr) => {
            let ty = synthesize_expression(db, tables, expr, &env, ctx)?;
            ensure_or_accumulate!(
                ty == *return_type,
                db,
                TypeError::ReturnTypeMismatch {
                    function: function_name.to_string(),
                    expected: return_type.name(),
                    found: ty.name(),
                    span: expr.span(),
                    file_id: ctx.file_id,
                }
            );
            Some(env)
        }
    }
}

fn check_expression(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    expression: &Expression,
    expected: &RT,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<()> {
    match expression {
        Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            check_boolean_condition(db, tables, condition, env, ctx)?;
            check_expression(db, tables, then_expr, expected, env, ctx)?;
            check_expression(db, tables, else_expr, expected, env, ctx)
        }
        _ => {
            let got = synthesize_expression(db, tables, expression, env, ctx)?;
            ensure_or_accumulate!(
                got == *expected,
                db,
                TypeError::TypeMismatch {
                    expected: expected.name(),
                    found: got.name(),
                    span: expression.span(),
                    file_id: ctx.file_id,
                }
            );
            Some(())
        }
    }
}

fn check_boolean_condition(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    condition: &Expression,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<()> {
    check_expression(db, tables, condition, &RT::boolean(), env, ctx)
}

fn check_block(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    stmts: &[Statement],
    mut env: TypeEnvironment,
    function_name: &str,
    return_type: &RT,
    ctx: &CheckContext,
) -> Option<()> {
    for stmt in stmts {
        env = check_statement(db, tables, stmt, env, function_name, return_type, ctx)?;
    }
    Some(())
}

pub(super) fn synthesize_expression(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    expression: &Expression,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<RT> {
    match expression {
        Expression::Call {
            function,
            arguments,
            span,
        } => synthesize_call(db, tables, function, arguments, *span, env, ctx),
        Expression::Variable { name, span } => env.lookup_variable(name).or_accumulate(
            db,
            TypeError::UnknownVariable {
                name: name.clone(),
                span: *span,
                file_id: ctx.file_id,
            },
        ),
        Expression::StringLiteral { .. } => Some(RT::string()),
        Expression::BooleanLiteral { .. } => Some(RT::boolean()),
        Expression::IntLiteral { .. } => Some(RT::int()),
        Expression::UnitLiteral { .. } => Some(RT::unit()),
        Expression::Placeholder { span } => {
            TypeError::TypeMismatch {
                expected: "concrete type".to_string(),
                found: "placeholder".to_string(),
                span: *span,
                file_id: ctx.file_id,
            }
            .accumulate(db);
            None
        }
        Expression::ListLiteral { elements, span } => {
            synthesize_list_literal(db, tables, elements, *span, env, ctx)
        }
        Expression::Select(select_expr) => {
            synthesize_select(db, tables, &select_expr.clauses, select_expr.span, env, ctx)
        }
        Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            span,
        } => synthesize_if_else(db, tables, condition, then_expr, else_expr, *span, env, ctx),
        Expression::StructLiteral {
            struct_name,
            fields,
            span,
        } => synthesize_struct_literal(db, tables, struct_name, fields, *span, env, ctx),
        Expression::FieldAccess { base, field, span } => {
            synthesize_field_access(db, tables, base, field, *span, env, ctx)
        }
    }
}

fn synthesize_call(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    function: &str,
    arguments: &[Expression],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<RT> {
    let interned_current = ctx.module_name.intern(db);
    let interned_fn = function.intern(db);

    let (resolved_fn_name, sig) = resolve_function_call(db, tables, interned_current, interned_fn)
        .and_then(|fn_name| {
            let interned = fn_name.clone().intern(db);
            get_function_sig(db, tables, interned).map(|arc| (fn_name, arc.get().clone()))
        })
        .or_accumulate(
            db,
            TypeError::UnknownFunction {
                name: function.to_string(),
                span,
                file_id: ctx.file_id,
            },
        )?;

    check_visibility(db, tables, &resolved_fn_name, span, ctx)?;

    ensure_or_accumulate!(
        arguments.len() == sig.parameters.len(),
        db,
        TypeError::ArgumentCountMismatch {
            function: function.to_string(),
            expected: sig.parameters.len(),
            found: arguments.len(),
            span,
            file_id: ctx.file_id,
        }
    );

    let mut unifier = Unifier::new();
    for (arg, param) in arguments.iter().zip(&sig.parameters) {
        if matches!(arg, Expression::Placeholder { .. }) {
            continue;
        }
        let arg_ty = synthesize_expression(db, tables, arg, env, ctx)?;
        ensure_or_accumulate!(
            unifier.unify_type(&param.param_type, &arg_ty).is_ok(),
            db,
            TypeError::ArgumentTypeMismatch {
                function: function.to_string(),
                parameter: param.name.clone(),
                expected: param.param_type.name(),
                found: arg_ty.name(),
                span: arg.span(),
                file_id: ctx.file_id,
            }
        );
    }
    for tp in &sig.type_params {
        if tp.bounds.is_empty() {
            continue;
        }
        unimplemented!("Removed a old impl was hacky as hell");
    }
    Some(unifier.apply_subst(&sig.return_type))
}

fn synthesize_list_literal(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    elements: &[Expression],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<RT> {
    ensure_or_accumulate!(
        !elements.is_empty(),
        db,
        TypeError::TypeMismatch {
            expected: "non-empty list or type annotation".to_string(),
            found: "empty list".to_string(),
            span,
            file_id: ctx.file_id,
        }
    );
    let first_type = synthesize_expression(db, tables, &elements[0], env, ctx)?;
    for elem in elements.iter().skip(1) {
        check_expression(db, tables, elem, &first_type, env, ctx)?;
    }
    Some(RT::list(first_type))
}

fn synthesize_select(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    clauses: &[SelectClause],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<RT> {
    ensure_or_accumulate!(
        !clauses.is_empty(),
        db,
        TypeError::TypeMismatch {
            expected: "non-empty select".to_string(),
            found: "empty select".to_string(),
            span,
            file_id: ctx.file_id,
        }
    );
    let first = &clauses[0];
    let first_result_type = synthesize_expression(db, tables, &first.expression_to_run, env, ctx)?;
    let mut first_env = env.create_child();
    first_env.declare_variable(
        first.result_variable.clone(),
        first_result_type,
        first.expression_to_run.span(),
    );
    let first_type = synthesize_expression(db, tables, &first.expression_next, &first_env, ctx)?;
    for (i, clause) in clauses.iter().enumerate().skip(1) {
        let result_type = synthesize_expression(db, tables, &clause.expression_to_run, env, ctx)?;
        let mut clause_env = env.create_child();
        clause_env.declare_variable(
            clause.result_variable.clone(),
            result_type,
            clause.expression_to_run.span(),
        );
        ensure_or_accumulate!(
            check_expression(
                db,
                tables,
                &clause.expression_next,
                &first_type,
                &clause_env,
                ctx,
            )
            .is_some(),
            db,
            TypeError::SelectBranchTypeMismatch {
                expected: first_type.name(),
                found: synthesize_expression(db, tables, &clause.expression_next, &clause_env, ctx)
                    .map(|t| t.name())
                    .unwrap_or_default(),
                branch_index: i,
                span: clause.expression_next.span(),
                first_branch_span: first.expression_next.span(),
                file_id: ctx.file_id,
            }
        );
    }
    Some(first_type)
}

fn synthesize_if_else(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    condition: &Expression,
    then_expr: &Expression,
    else_expr: &Expression,
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<RT> {
    let _ = span;
    check_boolean_condition(db, tables, condition, env, ctx)?;
    let then_type = synthesize_expression(db, tables, then_expr, env, ctx)?;
    check_expression(db, tables, else_expr, &then_type, env, ctx)?;
    Some(then_type)
}

fn synthesize_struct_literal(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    struct_name: &str,
    fields: &[(String, Expression)],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<RT> {
    let (definition, type_params) = get_struct_fields(db, tables, struct_name, ctx.module_name)
        .or_accumulate(
            db,
            TypeError::UnsupportedType {
                type_name: struct_name.to_string(),
                span,
                file_id: ctx.file_id,
            },
        )?;
    let mut seen = std::collections::HashSet::new();
    let mut unifier = Unifier::new();
    let type_env = super::TypeEnvironment::with_type_params(&type_params);
    for (field_name, value_expr) in fields {
        ensure_or_accumulate!(
            seen.insert(field_name.clone()),
            db,
            TypeError::DuplicateField {
                struct_name: struct_name.to_string(),
                field_name: field_name.clone(),
                span: value_expr.span(),
                file_id: ctx.file_id,
            }
        );
        let declared_ast_type = definition
            .iter()
            .find(|(n, _)| n == field_name)
            .map(|(_, t)| t.clone())
            .or_accumulate(
                db,
                TypeError::UnknownField {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                },
            )?;
        let declared_type = super::constraints::resolve(
            db,
            tables,
            &declared_ast_type,
            &type_env,
            value_expr.span(),
            ctx,
        )?;
        let value_type = synthesize_expression(db, tables, value_expr, env, ctx)?;
        ensure_or_accumulate!(
            unifier.unify_type(&declared_type, &value_type).is_ok(),
            db,
            TypeError::StructFieldTypeMismatch {
                struct_name: struct_name.to_string(),
                field_name: field_name.clone(),
                expected: declared_type.name(),
                found: value_type.name(),
                span: value_expr.span(),
                file_id: ctx.file_id,
            }
        );
    }
    for (required_field, _) in &definition {
        ensure_or_accumulate!(
            fields.iter().any(|(n, _)| n == required_field),
            db,
            TypeError::MissingField {
                struct_name: struct_name.to_string(),
                field_name: required_field.clone(),
                span,
                file_id: ctx.file_id,
            }
        );
    }
    let resolved_type_name = {
        let interned_mod = ctx.module_name.intern(db);
        let interned_name = struct_name.intern(db);
        resolve_type_alias(db, tables, interned_mod, interned_name).unwrap_or_else(|| TypeName {
            name: struct_name.to_string(),
            module: ctx.module_name.clone(),
        })
    };
    if type_params.is_empty() {
        Some(RT::Struct(resolved_type_name))
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
        Some(RT::Parameterized(resolved_type_name, args))
    }
}

fn synthesize_field_access(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    base: &Expression,
    field: &str,
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<RT> {
    let base_type = synthesize_expression(db, tables, base, env, ctx)?;
    match base_type {
        RT::Struct(type_name) => {
            let (definition, type_params) =
                get_struct_fields(db, tables, &type_name.name, ctx.module_name).or_accumulate(
                    db,
                    TypeError::UnsupportedType {
                        type_name: type_name.name.clone(),
                        span,
                        file_id: ctx.file_id,
                    },
                )?;
            let type_env = super::TypeEnvironment::with_type_params(&type_params);
            let field_ast_type = definition
                .iter()
                .find(|(n, _)| n == field)
                .map(|(_, t)| t.clone())
                .or_accumulate(
                    db,
                    TypeError::UnknownField {
                        struct_name: type_name.name.clone(),
                        field_name: field.to_string(),
                        span,
                        file_id: ctx.file_id,
                    },
                )?;
            super::constraints::resolve(db, tables, &field_ast_type, &type_env, span, ctx)
        }
        RT::Generic(name) => {
            let (definition, type_params) = get_struct_fields(db, tables, &name, ctx.module_name)
                .or_accumulate(
                db,
                TypeError::UnsupportedType {
                    type_name: name.clone(),
                    span,
                    file_id: ctx.file_id,
                },
            )?;
            let type_env = super::TypeEnvironment::with_type_params(&type_params);
            let field_ast_type = definition
                .iter()
                .find(|(n, _)| n == field)
                .map(|(_, t)| t.clone())
                .or_accumulate(
                    db,
                    TypeError::UnknownField {
                        struct_name: name.clone(),
                        field_name: field.to_string(),
                        span,
                        file_id: ctx.file_id,
                    },
                )?;
            super::constraints::resolve(db, tables, &field_ast_type, &type_env, span, ctx)
        }
        other => {
            TypeError::TypeMismatch {
                expected: "struct".to_string(),
                found: other.name(),
                span,
                file_id: ctx.file_id,
            }
            .accumulate(db);
            None
        }
    }
}
fn check_visibility(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    fn_name: &FunctionName,
    span: Span,
    ctx: &CheckContext,
) -> Option<()> {
    if &fn_name.module == ctx.module_name {
        return Some(());
    }
    let interned = fn_name.intern(db);
    let is_visible = lookup_function_def(db, tables, interned)
        .map(|arc_ptr| matches!(arc_ptr.get().visibility, Visibility::Public))
        .unwrap_or(true);
    ensure_or_accumulate!(
        is_visible,
        db,
        TypeError::PrivateFunction {
            name: format!("{}::{}", fn_name.module, fn_name.name),
            span,
            file_id: ctx.file_id,
        }
    );
    Some(())
}

pub(super) fn elaborate_function(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    func: &Function,
    ctx: &CheckContext,
    self_type: Option<structured_agent_runtime::symbols::TypeName>,
) -> Option<typed_ast::Function> {
    let mut env = TypeEnvironment::with_type_params(&func.type_params);
    if let Some(st) = self_type {
        env.set_self_type(st);
    }
    let mut typed_parameters = Vec::new();
    for param in &func.parameters {
        let runtime_type =
            super::constraints::resolve(db, tables, &param.param_type, &env, param.span, ctx)?;
        env.declare_variable(param.name.clone(), runtime_type.clone(), param.span);
        typed_parameters.push(typed_ast::Parameter {
            name: param.name.clone(),
            param_type: runtime_type,
            span: param.span,
        });
    }
    let runtime_return_type =
        super::constraints::resolve(db, tables, &func.return_type, &env, func.span, ctx)?;
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
    mut env: TypeEnvironment,
    ctx: &CheckContext,
) -> Option<(typed_ast::Statement, TypeEnvironment)> {
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
    stmts: &[Statement],
    mut env: TypeEnvironment,
    ctx: &CheckContext,
) -> Option<Vec<typed_ast::Statement>> {
    let mut typed_stmts = Vec::new();
    for stmt in stmts {
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
    env: &TypeEnvironment,
    ctx: &CheckContext,
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
        Expression::Select(select_expr) => {
            elaborate_select(db, tables, &select_expr.clauses, select_expr.span, env, ctx)
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
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<typed_ast::Expression> {
    let interned_current = ctx.module_name.intern(db);
    let interned_fn = function.intern(db);

    let (resolved_fn_name, sig) = resolve_function_call(db, tables, interned_current, interned_fn)
        .and_then(|fn_name| {
            let interned = fn_name.clone().intern(db);
            get_function_sig(db, tables, interned).map(|arc| (fn_name, arc.get().clone()))
        })?;

    let mut typed_args = Vec::new();

    let mut unifier = Unifier::new();
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
    Some(typed_ast::Expression::Call {
        function: function.to_string(),
        resolved: resolved_fn_name,
        kind: sig.kind,
        arguments: typed_args,
        ty: resolved_return,
        span,
    })
}

fn elaborate_list_literal(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    elements: &[Expression],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
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
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<typed_ast::Expression> {
    if clauses.is_empty() {
        return None;
    }
    let first = &clauses[0];
    let typed_first_run = elaborate_expression(db, tables, &first.expression_to_run, env, ctx)?;
    let first_result_type = typed_first_run.ty().clone();
    let mut first_env = env.create_child();
    first_env.declare_variable(
        first.result_variable.clone(),
        first_result_type,
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
        let result_type = typed_run.ty().clone();
        let mut clause_env = env.create_child();
        clause_env.declare_variable(
            clause.result_variable.clone(),
            result_type,
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
    env: &TypeEnvironment,
    ctx: &CheckContext,
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
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<typed_ast::Expression> {
    let (definition, type_params) = get_struct_fields(db, tables, struct_name, ctx.module_name)?;
    let type_env = super::TypeEnvironment::with_type_params(&type_params);
    let mut typed_fields = Vec::new();
    let mut unifier = Unifier::new();
    for (field_name, value_expr) in fields {
        let declared_ast_type = definition
            .iter()
            .find(|(n, _)| n == field_name)
            .map(|(_, t)| t.clone())?;
        let declared_type = super::constraints::resolve(
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
        resolve_type_alias(db, tables, interned_mod, interned_name).unwrap_or_else(|| TypeName {
            name: struct_name.to_string(),
            module: ctx.module_name.clone(),
        })
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
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<typed_ast::Expression> {
    let typed_base = elaborate_expression(db, tables, base, env, ctx)?;
    let base_type = typed_base.ty().clone();
    let struct_type_name = match &base_type {
        RT::Struct(tn) => tn.name.clone(),
        RT::Generic(name) => name.clone(),
        _ => return None,
    };
    let (definition, _) = get_struct_fields(db, tables, &struct_type_name, ctx.module_name)?;
    let field_ast_type = definition
        .iter()
        .find(|(n, _)| n == field)
        .map(|(_, t)| t.clone())?;
    let empty_env = super::TypeEnvironment::new();
    let field_ty = super::constraints::resolve(db, tables, &field_ast_type, &empty_env, span, ctx)?;
    Some(typed_ast::Expression::FieldAccess {
        base: Box::new(typed_base),
        field: field.to_string(),
        ty: field_ty,
        span,
    })
}
