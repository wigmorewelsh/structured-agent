use super::TypeChecker;
use super::db::{
    Intern, SymbolTablesInput, TypeCheckDatabase, resolve_function_call, resolve_type_alias,
};
use super::{CheckContext, TypeEnvironment};
use crate::ast::{
    Definition, Expression, Function, Parameter, SelectClause, Statement, Type as AstType,
};
use crate::typecheck::error::TypeError;
use crate::typed_ast;
use crate::types::{Span, Spanned};
use std::collections::HashMap;
use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::TypeName;

pub(super) fn check_definition(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    definition: &Definition,
    ctx: &CheckContext,
) -> Result<typed_ast::Definition, TypeError> {
    match definition {
        Definition::Function(func) => Ok(typed_ast::Definition::Function(check_function(
            db, tables, func, ctx,
        )?)),
        Definition::ExternalFunction(f) => {
            let module = ctx.module_name.clone();
            super::constraints::resolve(
                db,
                tables,
                &f.return_type,
                &module,
                &f.type_params,
                f.span,
                ctx.file_id,
            )?;
            for param in &f.parameters {
                super::constraints::resolve(
                    db,
                    tables,
                    &param.param_type,
                    &module,
                    &f.type_params,
                    param.span,
                    ctx.file_id,
                )?;
            }
            Ok(typed_ast::Definition::ExternalFunction((**f).clone()))
        }
        Definition::Struct(s) => {
            for f in &s.fields {
                super::constraints::resolve(
                    db,
                    tables,
                    &f.field_type,
                    ctx.module_name,
                    &s.type_params,
                    f.span,
                    ctx.file_id,
                )?;
            }
            Ok(typed_ast::Definition::Struct((**s).clone()))
        }
        Definition::Use {
            path,
            name,
            alias,
            is_pub,
            span,
        } => Ok(typed_ast::Definition::Use {
            path: path.clone(),
            name: name.clone(),
            alias: alias.clone(),
            is_pub: *is_pub,
            span: *span,
        }),
        Definition::ModuleBinding {
            name,
            sig_path,
            sig_name,
            impl_path,
            span,
        } => Ok(typed_ast::Definition::ModuleBinding {
            name: name.clone(),
            sig_path: sig_path.clone(),
            sig_name: sig_name.clone(),
            impl_path: impl_path.clone(),
            span: *span,
        }),
        Definition::WiringSite { name, args, span } => Ok(typed_ast::Definition::WiringSite {
            name: name.clone(),
            args: args.clone(),
            span: *span,
        }),
        Definition::Trait(s) => Ok(typed_ast::Definition::Trait {
            name: s.name.clone(),
            functions: s.functions.clone(),
            span: s.span,
        }),
        Definition::TraitImpl(t) => {
            let (type_name, trait_name, functions, span) =
                (&t.type_name, &t.trait_name, &t.functions, &t.span);
            let trait_fns =
                super::query::get_trait_functions(db, tables, trait_name, ctx.module_name);
            if let Some(trait_fns) = trait_fns {
                for trait_fn in &trait_fns {
                    if !functions.iter().any(|f| f.name == trait_fn.name) {
                        return Err(TypeError::TraitImplMissingFunction {
                            type_name: type_name.clone(),
                            trait_name: trait_name.clone(),
                            function_name: trait_fn.name.clone(),
                            span: *span,
                            file_id: ctx.file_id,
                        });
                    }
                }
            } else {
                return Err(TypeError::UnknownTrait {
                    name: trait_name.clone(),
                    span: *span,
                    file_id: ctx.file_id,
                });
            }
            let typed_functions: Result<Vec<typed_ast::Function>, TypeError> = functions
                .iter()
                .map(|func| {
                    let concrete = TypeChecker::substitute_self_in_fn(func, type_name);
                    check_function(db, tables, &concrete, ctx)
                })
                .collect();
            Ok(typed_ast::Definition::TraitImpl {
                type_name: type_name.clone(),
                trait_name: trait_name.clone(),
                functions: typed_functions?,
                span: *span,
            })
        }
        Definition::ModuleHeader { .. } | Definition::Signature(_) => unreachable!(),
    }
}

pub(super) fn check_function(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    func: &Function,
    ctx: &CheckContext,
) -> Result<typed_ast::Function, TypeError> {
    let mut env = TypeEnvironment::new();
    let module = ctx.module_name.clone();
    let mut typed_parameters = Vec::new();
    for param in &func.parameters {
        let runtime_type = super::constraints::resolve(
            db,
            tables,
            &param.param_type,
            &module,
            &func.type_params,
            param.span,
            ctx.file_id,
        )?;
        env.declare_variable(param.name.clone(), runtime_type.clone(), param.span);
        typed_parameters.push(typed_ast::Parameter {
            name: param.name.clone(),
            param_type: runtime_type,
            span: param.span,
        });
    }
    let runtime_return_type = super::constraints::resolve(
        db,
        tables,
        &func.return_type,
        &module,
        &func.type_params,
        func.span,
        ctx.file_id,
    )?;
    let mut typed_stmts = Vec::new();
    for statement in &func.body.statements {
        let (typed_stmt, new_env) = check_statement(
            db,
            tables,
            statement,
            env,
            &func.name,
            &runtime_return_type,
            ctx,
        )?;
        typed_stmts.push(typed_stmt);
        env = new_env;
    }
    Ok(typed_ast::Function {
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

fn check_statement(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    statement: &Statement,
    mut env: TypeEnvironment,
    function_name: &str,
    return_type: &RT,
    ctx: &CheckContext,
) -> Result<(typed_ast::Statement, TypeEnvironment), TypeError> {
    match statement {
        Statement::Injection(expr) => {
            let typed_expr = check_expression(db, tables, expr, &env, ctx)?;
            Ok((typed_ast::Statement::Injection(typed_expr), env))
        }
        Statement::Assignment {
            variable,
            expression,
            span,
        } => {
            let typed_expr = check_expression(db, tables, expression, &env, ctx)?;
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
            let typed_expr = check_expression(db, tables, expression, &env, ctx)?;
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
                    expected: existing_type.name(),
                    found: expr_type.name(),
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
            let typed_expr = check_expression(db, tables, expr, &env, ctx)?;
            Ok((typed_ast::Statement::ExpressionStatement(typed_expr), env))
        }
        Statement::If {
            condition,
            body,
            else_body,
            span,
        } => {
            let typed_condition = check_boolean_condition(db, tables, condition, &env, ctx)?;
            let typed_body = check_block(
                db,
                tables,
                body,
                env.create_child(),
                function_name,
                return_type,
                ctx,
            )?;
            let typed_else = if let Some(else_stmts) = else_body {
                Some(check_block(
                    db,
                    tables,
                    else_stmts,
                    env.create_child(),
                    function_name,
                    return_type,
                    ctx,
                )?)
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
            let typed_condition = check_boolean_condition(db, tables, condition, &env, ctx)?;
            let typed_body = check_block(
                db,
                tables,
                body,
                env.create_child(),
                function_name,
                return_type,
                ctx,
            )?;
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
            let typed_expr = check_expression(db, tables, expr, &env, ctx)?;
            if *typed_expr.ty() != *return_type {
                return Err(TypeError::ReturnTypeMismatch {
                    function: function_name.to_string(),
                    expected: return_type.name(),
                    found: typed_expr.ty().name(),
                    span: expr.span(),
                    file_id: ctx.file_id,
                });
            }
            Ok((typed_ast::Statement::Return(typed_expr), env))
        }
    }
}

fn check_boolean_condition(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    condition: &Expression,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Result<typed_ast::Expression, TypeError> {
    let typed_cond = check_expression(db, tables, condition, env, ctx)?;
    if matches!(typed_cond.ty(), RT::Boolean) {
        Ok(typed_cond)
    } else {
        Err(TypeError::TypeMismatch {
            expected: "Boolean".to_string(),
            found: typed_cond.ty().name(),
            span: condition.span(),
            file_id: ctx.file_id,
        })
    }
}

fn check_block(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    stmts: &[Statement],
    mut env: TypeEnvironment,
    function_name: &str,
    return_type: &RT,
    ctx: &CheckContext,
) -> Result<Vec<typed_ast::Statement>, TypeError> {
    let mut typed_stmts = Vec::new();
    for stmt in stmts {
        let (typed_stmt, new_env) =
            check_statement(db, tables, stmt, env, function_name, return_type, ctx)?;
        typed_stmts.push(typed_stmt);
        env = new_env;
    }
    Ok(typed_stmts)
}

pub(super) fn check_expression(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    expression: &Expression,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Result<typed_ast::Expression, TypeError> {
    match expression {
        Expression::Call {
            function,
            arguments,
            span,
        } => check_call(db, tables, function, arguments, *span, env, ctx),
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
            ty: RT::String,
            span: *span,
        }),
        Expression::BooleanLiteral { value, span } => Ok(typed_ast::Expression::BooleanLiteral {
            value: *value,
            ty: RT::Boolean,
            span: *span,
        }),
        Expression::IntLiteral { value, span } => Ok(typed_ast::Expression::IntLiteral {
            value: *value,
            ty: RT::Int,
            span: *span,
        }),
        Expression::UnitLiteral { span } => Ok(typed_ast::Expression::UnitLiteral {
            ty: RT::Unit,
            span: *span,
        }),
        Expression::Placeholder { span } => Err(TypeError::TypeMismatch {
            expected: "concrete type".to_string(),
            found: "placeholder".to_string(),
            span: *span,
            file_id: ctx.file_id,
        }),
        Expression::ListLiteral { elements, span } => {
            check_list_literal(db, tables, elements, *span, env, ctx)
        }
        Expression::Select(select_expr) => {
            check_select(db, tables, &select_expr.clauses, select_expr.span, env, ctx)
        }
        Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            span,
        } => check_if_else_expression(db, tables, condition, then_expr, else_expr, *span, env, ctx),
        Expression::StructLiteral {
            struct_name,
            fields,
            span,
        } => check_struct_literal(db, tables, struct_name, fields, *span, env, ctx),
        Expression::FieldAccess { base, field, span } => {
            check_field_access(db, tables, base, field, *span, env, ctx)
        }
    }
}

fn check_call(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    function: &str,
    arguments: &[Expression],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Result<typed_ast::Expression, TypeError> {
    let interned_current = ctx.module_name.intern(db);
    let interned_fn = function.intern(db);

    let (resolved_fn_name, sig) = resolve_function_call(db, tables, interned_current, interned_fn)
        .and_then(|fn_name| {
            super::query::get_function_sig(db, tables, &fn_name).map(|sig| (fn_name, sig))
        })
        .or_else(|| super::query::resolve_impl_call(db, tables, function, arguments, env, ctx))
        .ok_or_else(|| TypeError::UnknownFunction {
            name: function.to_string(),
            span,
            file_id: ctx.file_id,
        })?;

    super::query::check_visibility(db, tables, &resolved_fn_name, span, ctx)?;

    let (kind, return_type, parameters, type_params) =
        (sig.kind, sig.return_type, sig.parameters, sig.type_params);

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
            let typed_arg = check_expression(db, tables, arg, env, ctx)?;
            if typed_arg.ty() != &param.param_type {
                return Err(TypeError::ArgumentTypeMismatch {
                    function: function.to_string(),
                    parameter: param.name.clone(),
                    expected: param.param_type.name(),
                    found: typed_arg.ty().name(),
                    span: arg.span(),
                    file_id: ctx.file_id,
                });
            }
            typed_args.push(typed_arg);
        }

        Ok(typed_ast::Expression::Call {
            function: function.to_string(),
            resolved: resolved_fn_name,
            kind,
            arguments: typed_args,
            ty: return_type,
            span,
        })
    } else {
        let mut subst: HashMap<String, RT> = HashMap::new();
        for (arg, param) in arguments.iter().zip(&parameters) {
            if matches!(arg, Expression::Placeholder { .. }) {
                typed_args.push(typed_ast::Expression::Placeholder {
                    ty: param.param_type.clone(),
                    span: arg.span(),
                });
                continue;
            }
            let typed_arg = check_expression(db, tables, arg, env, ctx)?;
            if !TypeChecker::unify_type(&param.param_type, typed_arg.ty(), &mut subst) {
                let expected = TypeChecker::apply_subst(&param.param_type, &subst);
                return Err(TypeError::ArgumentTypeMismatch {
                    function: function.to_string(),
                    parameter: param.name.clone(),
                    expected: expected.name(),
                    found: typed_arg.ty().name(),
                    span: arg.span(),
                    file_id: ctx.file_id,
                });
            }
            typed_args.push(typed_arg);
        }

        for tp in &type_params {
            if tp.bounds.is_empty() {
                continue;
            }
            if let Some(concrete) = subst.get(&tp.name) {
                let type_name = match concrete {
                    RT::Int => "Int",
                    RT::String => "String",
                    RT::Boolean => "Boolean",
                    RT::Struct(n) => n.name.as_str(),
                    _ => continue,
                };
                for bound in &tp.bounds {
                    let trait_name = match bound {
                        AstType::Struct(n) => n.as_str(),
                        AstType::Generic(n) => n.as_str(),
                        _ => continue,
                    };
                    let satisfied =
                        super::query::type_implements_trait(db, tables, type_name, trait_name);
                    if !satisfied {
                        return Err(TypeError::TraitBoundNotSatisfied {
                            type_name: type_name.to_string(),
                            trait_name: trait_name.to_string(),
                            param_name: tp.name.clone(),
                            span,
                            file_id: ctx.file_id,
                        });
                    }
                }
            }
        }

        let resolved_return = TypeChecker::apply_subst(&return_type, &subst);
        Ok(typed_ast::Expression::Call {
            function: function.to_string(),
            resolved: resolved_fn_name,
            kind,
            arguments: typed_args,
            ty: resolved_return,
            span,
        })
    }
}

fn check_list_literal(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
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

    let typed_first = check_expression(db, tables, &elements[0], env, ctx)?;
    let first_type = typed_first.ty().clone();
    let mut typed_elements = vec![typed_first];

    for elem in elements.iter().skip(1) {
        let typed_elem = check_expression(db, tables, elem, env, ctx)?;
        if *typed_elem.ty() != first_type {
            return Err(TypeError::TypeMismatch {
                expected: first_type.name(),
                found: typed_elem.ty().name(),
                span: elem.span(),
                file_id: ctx.file_id,
            });
        }
        typed_elements.push(typed_elem);
    }

    Ok(typed_ast::Expression::ListLiteral {
        elements: typed_elements,
        ty: RT::list(first_type),
        span,
    })
}

fn check_select(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
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
    let typed_first_run = check_expression(db, tables, &first.expression_to_run, env, ctx)?;
    let first_result_type = typed_first_run.ty().clone();
    let mut first_env = env.create_child();
    first_env.declare_variable(
        first.result_variable.clone(),
        first_result_type,
        first.expression_to_run.span(),
    );
    let typed_first_next = check_expression(db, tables, &first.expression_next, &first_env, ctx)?;
    let first_type = typed_first_next.ty().clone();

    let mut typed_clauses = vec![typed_ast::SelectClause {
        expression_to_run: typed_first_run,
        result_variable: first.result_variable.clone(),
        expression_next: typed_first_next,
        span: first.span,
    }];

    for (i, clause) in clauses.iter().enumerate().skip(1) {
        let typed_run = check_expression(db, tables, &clause.expression_to_run, env, ctx)?;
        let result_type = typed_run.ty().clone();
        let mut clause_env = env.create_child();
        clause_env.declare_variable(
            clause.result_variable.clone(),
            result_type,
            clause.expression_to_run.span(),
        );
        let typed_next = check_expression(db, tables, &clause.expression_next, &clause_env, ctx)?;
        if first_type != *typed_next.ty() {
            return Err(TypeError::SelectBranchTypeMismatch {
                expected: first_type.name(),
                found: typed_next.ty().name(),
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
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    condition: &Expression,
    then_expr: &Expression,
    else_expr: &Expression,
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Result<typed_ast::Expression, TypeError> {
    let typed_condition = check_boolean_condition(db, tables, condition, env, ctx)?;
    let typed_then = check_expression(db, tables, then_expr, env, ctx)?;
    let typed_else = check_expression(db, tables, else_expr, env, ctx)?;

    if typed_then.ty() != typed_else.ty() {
        return Err(TypeError::TypeMismatch {
            expected: typed_then.ty().name(),
            found: typed_else.ty().name(),
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
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    struct_name: &str,
    fields: &[(String, Expression)],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Result<typed_ast::Expression, TypeError> {
    let (definition, type_params) =
        super::query::get_struct_fields(db, tables, struct_name, ctx.module_name).ok_or_else(
            || TypeError::UnsupportedType {
                type_name: struct_name.to_string(),
                span,
                file_id: ctx.file_id,
            },
        )?;

    let mut seen = std::collections::HashSet::new();
    let mut typed_fields = Vec::new();
    let mut subst: HashMap<String, RT> = HashMap::new();

    for (field_name, value_expr) in fields {
        if !seen.insert(field_name.clone()) {
            return Err(TypeError::DuplicateField {
                struct_name: struct_name.to_string(),
                field_name: field_name.clone(),
                span: value_expr.span(),
                file_id: ctx.file_id,
            });
        }

        let declared_ast_type = definition
            .iter()
            .find(|(n, _)| n == field_name)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| TypeError::UnknownField {
                struct_name: struct_name.to_string(),
                field_name: field_name.clone(),
                span: value_expr.span(),
                file_id: ctx.file_id,
            })?;

        let declared_type = super::constraints::resolve(
            db,
            tables,
            &declared_ast_type,
            ctx.module_name,
            &type_params,
            value_expr.span(),
            ctx.file_id,
        )?;
        let typed_value = check_expression(db, tables, value_expr, env, ctx)?;
        if !TypeChecker::unify_type(&declared_type, typed_value.ty(), &mut subst) {
            let expected = TypeChecker::apply_subst(&declared_type, &subst);
            return Err(TypeError::StructFieldTypeMismatch {
                struct_name: struct_name.to_string(),
                field_name: field_name.clone(),
                expected: expected.name(),
                found: typed_value.ty().name(),
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
                subst
                    .get(&tp.name)
                    .cloned()
                    .unwrap_or_else(|| RT::Generic(tp.name.clone()))
            })
            .collect();
        RT::Parameterized(resolved_type_name, args)
    };

    Ok(typed_ast::Expression::StructLiteral {
        struct_name: struct_name.to_string(),
        fields: typed_fields,
        ty,
        span,
    })
}

fn check_field_access(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    base: &Expression,
    field: &str,
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Result<typed_ast::Expression, TypeError> {
    let typed_base = check_expression(db, tables, base, env, ctx)?;
    let base_type = typed_base.ty().clone();
    match base_type {
        RT::Struct(type_name) => {
            let (definition, _) =
                super::query::get_struct_fields(db, tables, &type_name.name, ctx.module_name)
                    .ok_or_else(|| TypeError::UnsupportedType {
                        type_name: type_name.name.clone(),
                        span,
                        file_id: ctx.file_id,
                    })?;
            let field_ast_type = definition
                .iter()
                .find(|(n, _)| n == field)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| TypeError::UnknownField {
                    struct_name: type_name.name.clone(),
                    field_name: field.to_string(),
                    span,
                    file_id: ctx.file_id,
                })?;
            Ok(typed_ast::Expression::FieldAccess {
                base: Box::new(typed_base),
                field: field.to_string(),
                ty: super::constraints::resolve(
                    db,
                    tables,
                    &field_ast_type,
                    ctx.module_name,
                    &[],
                    span,
                    ctx.file_id,
                )?,
                span,
            })
        }
        RT::Generic(name) => {
            let (definition, _) =
                super::query::get_struct_fields(db, tables, &name, ctx.module_name).ok_or_else(
                    || TypeError::UnsupportedType {
                        type_name: name.clone(),
                        span,
                        file_id: ctx.file_id,
                    },
                )?;
            let field_ast_type = definition
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
                ty: super::constraints::resolve(
                    db,
                    tables,
                    &field_ast_type,
                    ctx.module_name,
                    &[],
                    span,
                    ctx.file_id,
                )?,
                span,
            })
        }
        other => Err(TypeError::TypeMismatch {
            expected: "struct".to_string(),
            found: other.name(),
            span,
            file_id: ctx.file_id,
        }),
    }
}

impl TypeChecker {
    pub(super) fn substitute_self(ty: &AstType, concrete: &str) -> AstType {
        match ty {
            AstType::Generic(name) if name == "Self" => AstType::Struct(concrete.to_string()),
            AstType::Parameterized(name, args) => AstType::Parameterized(
                name.clone(),
                args.iter()
                    .map(|a| Self::substitute_self(a, concrete))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    pub(super) fn substitute_self_in_fn(func: &Function, concrete: &str) -> Function {
        Function {
            name: func.name.clone(),
            parameters: func
                .parameters
                .iter()
                .map(|p| Parameter {
                    name: p.name.clone(),
                    param_type: Self::substitute_self(&p.param_type, concrete),
                    span: p.span,
                })
                .collect(),
            return_type: Self::substitute_self(&func.return_type, concrete),
            body: func.body.clone(),
            documentation: func.documentation.clone(),
            is_pub: func.is_pub,
            span: func.span,
            type_params: func.type_params.clone(),
        }
    }
}
