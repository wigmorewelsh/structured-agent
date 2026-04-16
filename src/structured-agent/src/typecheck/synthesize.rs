use super::db::{
    Intern, InternedModuleName, InternedString, SymbolTablesInput, TypeCheckDatabase,
    get_function_sig, get_struct_fields, lookup_function_def, resolve_function_call,
    resolve_type_alias,
};
use super::error::OrAccumulateError;
use crate::ast::{
    Definition, Expression, Function, SelectClause, Statement, Type as AstType, TypeParam,
};
use crate::ensure_or_accumulate;
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span, Spanned};

use std::collections::HashMap;
use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::{
    FunctionName, FunctionNameKind, ModuleName, TypeDefinitionKind, TypeName, Visibility,
};

#[derive(Debug, Clone)]
pub(crate) struct TypeEnvironment {
    pub(super) variables: HashMap<String, (structured_agent_runtime::Type, Span)>,
    pub(super) type_params: HashMap<String, ()>,
    pub(super) self_type: Option<structured_agent_runtime::symbols::TypeName>,
    pub(super) parent: Option<Box<TypeEnvironment>>,
}

pub(crate) struct CheckContext<'a> {
    pub(super) file_id: FileId,
    pub(super) module_name: &'a ModuleName,
}

impl TypeEnvironment {
    pub(super) fn new() -> Self {
        Self {
            variables: HashMap::new(),
            type_params: HashMap::new(),
            self_type: None,
            parent: None,
        }
    }

    pub(super) fn with_type_params(type_params: &[TypeParam]) -> Self {
        let mut env = Self::new();
        for tp in type_params {
            env.add_type_param(tp.name.clone());
        }
        env
    }

    pub(super) fn create_child(&self) -> Self {
        Self {
            variables: HashMap::new(),
            type_params: self.type_params.clone(),
            self_type: self.self_type.clone(),
            parent: Some(Box::new(self.clone())),
        }
    }

    fn add_type_param(&mut self, name: String) {
        self.type_params.insert(name, ());
    }

    pub(super) fn lookup_type_param(&self, name: &str) -> bool {
        if name == "Self" && self.self_type.is_some() {
            return true;
        }
        if self.type_params.contains_key(name) {
            return true;
        }
        if let Some(parent) = &self.parent {
            parent.lookup_type_param(name)
        } else {
            false
        }
    }

    pub(super) fn set_self_type(&mut self, self_type: structured_agent_runtime::symbols::TypeName) {
        self.self_type = Some(self_type);
    }

    pub(super) fn declare_variable(
        &mut self,
        name: String,
        var_type: structured_agent_runtime::Type,
        span: Span,
    ) {
        self.variables.insert(name, (var_type, span));
    }

    pub(super) fn lookup_variable(&self, name: &str) -> Option<structured_agent_runtime::Type> {
        if let Some((ty, _)) = self.variables.get(name) {
            Some(ty.clone())
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable(name)
        } else {
            None
        }
    }

    pub(super) fn lookup_variable_with_span(
        &self,
        name: &str,
    ) -> Option<(structured_agent_runtime::Type, Span)> {
        if let Some((ty, span)) = self.variables.get(name) {
            Some((ty.clone(), *span))
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable_with_span(name)
        } else {
            None
        }
    }
}

fn resolve_local_type<'db>(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    module: InternedModuleName<'db>,
    name: InternedString<'db>,
) -> Option<TypeName> {
    let local_type = TypeName {
        name: name.value(db).to_string(),
        module: module.name(db).clone(),
    };
    if tables.types(db).get().contains_key(&local_type) {
        Some(local_type)
    } else {
        None
    }
}

fn resolve_type_name(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &str,
    span: Span,
    ctx: &CheckContext,
) -> Option<TypeName> {
    let interned_mod = ctx.module_name.intern(db);
    let interned_name = name.intern(db);

    let type_name = resolve_local_type(db, tables, interned_mod, interned_name)
        .or_else(|| resolve_type_alias(db, tables, interned_mod, interned_name));

    type_name.or_accumulate(
        db,
        TypeError::UndefinedType {
            name: name.to_string(),
            span,
            file_id: ctx.file_id,
        },
    )
}

pub(super) fn resolve(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    t: &AstType,
    env: &TypeEnvironment,
    span: Span,
    ctx: &CheckContext,
) -> Option<RT> {
    let AstType { name, args } = t;

    if name == "Self" {
        if let Some(ref self_type) = env.self_type {
            return Some(RT::Struct(self_type.clone()));
        }
    }

    if env.lookup_type_param(name) {
        return Some(RT::Generic(name.to_string()));
    }

    let type_name = resolve_type_name(db, tables, name.as_str(), span, ctx)?;

    let types_table = tables.types(db);
    let td = types_table.get().get(&type_name).or_accumulate(
        db,
        TypeError::UndefinedType {
            name: name.clone(),
            span,
            file_id: ctx.file_id,
        },
    )?;

    match &td.kind {
        TypeDefinitionKind::Struct { .. } | TypeDefinitionKind::Primitive if args.is_empty() => {
            Some(RT::Struct(type_name))
        }
        TypeDefinitionKind::Native { .. } => {
            let inner_rt = resolve(db, tables, &args[0], env, span, ctx)?;
            Some(RT::Parameterized(type_name, vec![inner_rt]))
        }
        TypeDefinitionKind::Struct {
            generic_parameters, ..
        } => {
            let resolved_args: Vec<RT> = args
                .iter()
                .map(|a| resolve(db, tables, a, env, span, ctx))
                .collect::<Option<Vec<_>>>()?;
            ensure_or_accumulate!(
                resolved_args.len() == generic_parameters.len(),
                db,
                TypeError::UnboundTypeParameter {
                    name: name.clone(),
                    span,
                    file_id: ctx.file_id,
                }
            );
            Some(RT::Parameterized(type_name, resolved_args))
        }
        _ => {
            TypeError::UnboundTypeParameter {
                name: name.clone(),
                span,
                file_id: ctx.file_id,
            }
            .accumulate(db);
            None
        }
    }
}

pub struct Unifier {
    subst: HashMap<String, RT>,
}

impl Unifier {
    pub fn new() -> Self {
        Self {
            subst: HashMap::new(),
        }
    }

    pub fn unify_type(&mut self, formal: &RT, actual: &RT) -> Result<(), RT> {
        match formal {
            RT::Generic(name) => {
                if let Some(bound) = self.subst.get(name) {
                    if bound == actual {
                        Ok(())
                    } else {
                        Err(bound.clone())
                    }
                } else {
                    self.subst.insert(name.clone(), actual.clone());
                    Ok(())
                }
            }
            RT::Parameterized(name_formal, args_formal) => {
                if let RT::Parameterized(name_actual, args_actual) = actual {
                    if name_formal == name_actual
                        && args_formal.len() == args_actual.len()
                        && args_formal
                            .iter()
                            .zip(args_actual.iter())
                            .all(|(f, a)| self.unify_type(f, a).is_ok())
                    {
                        Ok(())
                    } else {
                        Err(self.apply_subst(formal))
                    }
                } else {
                    Err(self.apply_subst(formal))
                }
            }
            _ => {
                if formal == actual {
                    Ok(())
                } else {
                    Err(formal.clone())
                }
            }
        }
    }

    pub fn apply_subst(&self, ty: &RT) -> RT {
        match ty {
            RT::Generic(name) => self.subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            RT::Parameterized(name, args) => RT::Parameterized(
                name.clone(),
                args.iter().map(|a| self.apply_subst(a)).collect(),
            ),
            other => other.clone(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&RT> {
        self.subst.get(name)
    }
}

pub(super) fn check_definition(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    definition: &Definition,
    ctx: &CheckContext,
) -> Option<()> {
    match definition {
        Definition::Function(func) => check_function(db, tables, func, ctx),
        Definition::ExternalFunction(f) => {
            let env = TypeEnvironment::with_type_params(&f.type_params);
            resolve(db, tables, &f.return_type, &env, f.span, ctx)?;
            for param in &f.parameters {
                resolve(db, tables, &param.param_type, &env, param.span, ctx)?;
            }
            Some(())
        }
        Definition::Struct(s) => {
            let env = TypeEnvironment::with_type_params(&s.type_params);
            for f in &s.fields {
                resolve(db, tables, &f.field_type, &env, f.span, ctx)?;
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
    let type_env = TypeEnvironment::with_type_params(&type_params);
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
        let declared_type = resolve(
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
            resolve(db, tables, &field_ast_type, &type_env, span, ctx)
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
            resolve(db, tables, &field_ast_type, &type_env, span, ctx)
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
