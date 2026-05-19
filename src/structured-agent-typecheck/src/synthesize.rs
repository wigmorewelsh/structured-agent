use super::db::{
    Intern, InternedTypeName, TypeCheckDatabase, find_impl_fn_by_type_path, get_function_sig,
    get_struct_fields, lookup_function_def, lookup_type_def_in_symbol_tables,
    resolve_function_call, resolve_type_in_module,
};
use super::error::OrAccumulateError;
use crate::ensure_or_accumulate;
use crate::error::{TypeError, TypeErrorAccumulator};
use crate::solver::{Constraint, ConstraintKind, Flavour};
use structured_agent_ast::ast::{
    Definition, Expression, Function, MatchArm, SelectClause, Statement, StringPart,
    Type as AstType, TypeParam,
};
use structured_agent_ast::types::{FileId, Span, Spanned};
use structured_agent_typed_ast::BindingId;

use salsa::Accumulator;
use std::collections::HashMap;
use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::{
    DefinitionPath, FunctionKind, TypeDefinitionKind, Visibility,
};

use super::db::{InternedFunctionName, InternedModuleName};

#[derive(Debug, Clone)]
pub struct TypeEnvironment {
    pub variables: HashMap<String, (structured_agent_runtime::Type, BindingId, Span)>,
    pub type_params: HashMap<String, Vec<AstType>>,
    pub self_type: Option<DefinitionPath>,
    pub parent: Option<Box<TypeEnvironment>>,
    id_alloc: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

pub struct CheckContext<'a> {
    pub file_id: FileId,
    pub module_name: &'a DefinitionPath,
    pub program: crate::db::ProgramInput,
}

impl TypeEnvironment {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            type_params: HashMap::new(),
            self_type: None,
            parent: None,
            id_alloc: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    pub fn with_type_params(type_params: &[TypeParam]) -> Self {
        let mut env = Self::new();
        for tp in type_params {
            env.add_type_param(tp.name.clone(), tp.bounds.clone());
        }
        env
    }

    pub fn create_child(&self) -> Self {
        Self {
            variables: HashMap::new(),
            type_params: self.type_params.clone(),
            self_type: self.self_type.clone(),
            parent: Some(Box::new(self.clone())),
            id_alloc: std::sync::Arc::clone(&self.id_alloc),
        }
    }

    fn add_type_param(&mut self, name: String, bounds: Vec<AstType>) {
        self.type_params.insert(name, bounds);
    }

    pub fn get_type_param_bounds(&self, name: &str) -> Option<&Vec<AstType>> {
        if let Some(bounds) = self.type_params.get(name) {
            return Some(bounds);
        }
        if let Some(parent) = &self.parent {
            parent.get_type_param_bounds(name)
        } else {
            None
        }
    }

    pub fn lookup_type_param(&self, name: &str) -> bool {
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

    pub fn set_self_type(&mut self, self_type: DefinitionPath) {
        self.self_type = Some(self_type);
    }

    pub fn declare_variable(
        &mut self,
        name: String,
        var_type: structured_agent_runtime::Type,
        span: Span,
    ) -> BindingId {
        let id = BindingId(
            self.id_alloc
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        self.variables.insert(name, (var_type, id, span));
        id
    }

    pub fn lookup_variable(
        &self,
        name: &str,
    ) -> Option<(structured_agent_runtime::Type, BindingId)> {
        if let Some((ty, id, _)) = self.variables.get(name) {
            Some((ty.clone(), *id))
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable(name)
        } else {
            None
        }
    }

    pub fn lookup_variable_with_span(
        &self,
        name: &str,
    ) -> Option<(structured_agent_runtime::Type, BindingId, Span)> {
        if let Some((ty, id, span)) = self.variables.get(name) {
            Some((ty.clone(), *id, *span))
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable_with_span(name)
        } else {
            None
        }
    }
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_type_name(
    db: &dyn TypeCheckDatabase,
    name: &str,
    span: Span,
    ctx: &CheckContext,
) -> Option<DefinitionPath> {
    let interned_mod = InternedModuleName::new(db, ctx.module_name.clone());
    let interned_name = name.intern(db);

    let type_name = resolve_type_in_module(db, interned_mod, interned_name).map(|r| r.ty);

    type_name.or_accumulate(
        db,
        TypeError::UndefinedType {
            name: name.to_string(),
            span,
            file_id: ctx.file_id,
        },
    )
}

pub fn resolve(
    db: &dyn TypeCheckDatabase,
    t: &AstType,
    env: &TypeEnvironment,
    span: Span,
    ctx: &CheckContext,
) -> Option<RT> {
    let (name, args) = match t {
        AstType::Named { path, args } => (path.first().name.as_str().to_string(), args),
        AstType::Union(members) => {
            let resolved: Option<Vec<RT>> = members
                .iter()
                .map(|m| resolve(db, m, env, span, ctx))
                .collect();
            return resolved.map(RT::union);
        }
    };

    if name == "Self"
        && let Some(ref self_type) = env.self_type
    {
        return Some(RT::Named(self_type.clone()));
    }

    if env.lookup_type_param(&name) {
        return Some(RT::Generic(name.to_string()));
    }

    if name == "Option" && args.len() == 1 {
        let inner = resolve(db, &args[0], env, span, ctx)?;
        return Some(RT::union(vec![inner, RT::unit()]));
    }

    let type_name = resolve_type_name(db, name.as_str(), span, ctx)?;

    let types_table = db.symbol_tables().types(db);
    let td = types_table.get().get(&type_name).or_accumulate(
        db,
        TypeError::UndefinedType {
            name: name.clone(),
            span,
            file_id: ctx.file_id,
        },
    )?;

    match &td.kind {
        TypeDefinitionKind::Struct { .. }
        | TypeDefinitionKind::Primitive
        | TypeDefinitionKind::Signature { .. }
            if args.is_empty() =>
        {
            Some(RT::Named(type_name))
        }
        TypeDefinitionKind::Alias { ty } => resolve(db, ty, env, span, ctx),
        TypeDefinitionKind::Native { .. } => {
            let inner_rt = resolve(db, &args[0], env, span, ctx)?;
            Some(RT::Parameterized(type_name, vec![inner_rt]))
        }
        TypeDefinitionKind::Struct {
            generic_parameters, ..
        } => {
            let resolved_args: Vec<RT> = args
                .iter()
                .map(|a| resolve(db, a, env, span, ctx))
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
            TypeErrorAccumulator(TypeError::UnboundTypeParameter {
                name: name.clone(),
                span,
                file_id: ctx.file_id,
            })
            .accumulate(db);
            None
        }
    }
}

pub struct Substitution {
    subst: HashMap<String, RT>,
}

impl Substitution {
    pub fn new() -> Self {
        Self {
            subst: HashMap::new(),
        }
    }

    pub fn bind(&mut self, name: String, ty: RT) {
        self.subst.insert(name, ty);
    }

    pub fn extend(&mut self, bindings: HashMap<String, RT>) {
        self.subst.extend(bindings);
    }

    pub fn match_type(&mut self, formal: &RT, actual: &RT) -> Result<(), RT> {
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
                            .all(|(f, a)| self.match_type(f, a).is_ok())
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
                if actual.is_assignable_to(formal) {
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
            RT::Union(variants) => {
                RT::union(variants.iter().map(|v| self.apply_subst(v)).collect())
            }
            other => other.clone(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&RT> {
        self.subst.get(name)
    }
}

pub fn extract_generic_bindings(formal: &RT, actual: &RT) -> Option<HashMap<String, RT>> {
    match formal {
        RT::Generic(name) => {
            let mut m = HashMap::new();
            m.insert(name.clone(), actual.clone());
            Some(m)
        }
        RT::Parameterized(name_f, args_f) => {
            if let RT::Parameterized(name_a, args_a) = actual {
                if name_f == name_a && args_f.len() == args_a.len() {
                    let mut result = HashMap::new();
                    for (f, a) in args_f.iter().zip(args_a.iter()) {
                        match extract_generic_bindings(f, a) {
                            Some(inner) => result.extend(inner),
                            None => return None,
                        }
                    }
                    Some(result)
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => {
            if actual.is_assignable_to(formal) {
                Some(HashMap::new())
            } else {
                None
            }
        }
    }
}

pub fn check_definition(
    db: &dyn TypeCheckDatabase,
    definition: &Definition,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<()> {
    match definition {
        Definition::Function(func) => check_function(db, func, ctx, constraints),
        Definition::ExternalFunction(f) => {
            let env = TypeEnvironment::with_type_params(&f.type_params);
            resolve(db, &f.return_type, &env, f.span, ctx)?;
            for param in &f.parameters {
                resolve(db, &param.param_type, &env, param.span, ctx)?;
            }
            Some(())
        }
        Definition::Struct(s) => {
            let env = TypeEnvironment::with_type_params(&s.type_params);
            for f in &s.fields {
                resolve(db, &f.field_type, &env, f.span, ctx)?;
            }
            Some(())
        }
        Definition::Use(_) | Definition::Trait(_) | Definition::TypeAlias { .. } => Some(()),
        Definition::TraitImpl(t) => {
            let impls = db.symbol_tables().impls(db);
            let impl_entry = impls
                .get()
                .values()
                .find(|i| {
                    i.type_name.name() == t.type_name
                        && i.module == *ctx.module_name
                        && match &t.trait_name {
                            Some(tn) => i
                                .trait_name
                                .as_ref()
                                .map(|itn| itn.name() == tn.as_str())
                                .unwrap_or(false),
                            None => i.trait_name.is_none(),
                        }
                })
                .or_accumulate(
                    db,
                    TypeError::UnsupportedType {
                        type_name: t.type_name.clone(),
                        span: t.span,
                        file_id: ctx.file_id,
                    },
                )?;
            let impl_key = impl_entry.key.clone();
            for func in &t.functions {
                check_impl_function(db, func, &impl_key, ctx, constraints)?;
            }
            if let Some(trait_name_str) = &t.trait_name {
                let interned_mod = InternedModuleName::new(db, ctx.module_name.clone());
                let type_path =
                    resolve_type_in_module(db, interned_mod, t.type_name.clone().intern(db))
                        .map(|r| r.ty)
                        .or_accumulate(
                            db,
                            TypeError::UndefinedType {
                                name: t.type_name.clone(),
                                span: t.span,
                                file_id: ctx.file_id,
                            },
                        )?;
                let trait_path =
                    resolve_type_in_module(db, interned_mod, trait_name_str.clone().intern(db))
                        .and_then(|r| {
                            let types = db.symbol_tables().types(db);
                            if matches!(
                                types.get().get(&r.ty)?.kind,
                                TypeDefinitionKind::Trait { .. }
                            ) {
                                Some(r.ty)
                            } else {
                                None
                            }
                        })
                        .or_accumulate(
                            db,
                            TypeError::UnknownTrait {
                                name: trait_name_str.clone(),
                                span: t.span,
                                file_id: ctx.file_id,
                            },
                        )?;
                constraints.push(Constraint {
                    kind: ConstraintKind::TraitImpl {
                        type_path,
                        trait_path,
                        impl_key: impl_key.clone(),
                    },
                    span: t.span,
                    file_id: ctx.file_id,
                    flavour: Flavour::Wanted,
                });
            }
            Some(())
        }
        Definition::ModuleHeader { .. }
        | Definition::Signature(_)
        | Definition::InlineModule { .. } => unreachable!(),
    }
}

fn check_impl_function(
    db: &dyn TypeCheckDatabase,
    func: &Function,
    impl_key: &DefinitionPath,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<()> {
    let impl_fn_path = DefinitionPath::for_impl_fn(impl_key, func.name.clone());
    let sig = get_function_sig(db, InternedFunctionName::new(db, impl_fn_path), ctx.program)?
        .get()
        .clone();
    let mut env = TypeEnvironment::with_type_params(&sig.type_params);
    for param in &sig.parameters {
        env.declare_variable(param.name.clone(), param.param_type.clone(), param.span);
    }
    check_block(
        db,
        &func.body.statements,
        env,
        &func.name,
        &sig.return_type,
        ctx,
        constraints,
    )
}

fn check_function(
    db: &dyn TypeCheckDatabase,
    func: &Function,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<()> {
    let fn_name = DefinitionPath::for_function(ctx.module_name.clone(), func.name.clone());
    let sig = get_function_sig(db, InternedFunctionName::new(db, fn_name), ctx.program)?
        .get()
        .clone();
    let mut env = TypeEnvironment::with_type_params(&sig.type_params);
    for param in &sig.parameters {
        env.declare_variable(param.name.clone(), param.param_type.clone(), param.span);
    }
    check_block(
        db,
        &func.body.statements,
        env,
        &func.name,
        &sig.return_type,
        ctx,
        constraints,
    )
}

fn check_statement(
    db: &dyn TypeCheckDatabase,
    statement: &Statement,
    mut env: TypeEnvironment,
    function_name: &str,
    return_type: &RT,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<TypeEnvironment> {
    match statement {
        Statement::Yield { .. } => Some(env),
        Statement::Injection(expr) => {
            synthesize_expression(db, expr, &env, ctx, constraints)?;
            Some(env)
        }
        Statement::Assignment {
            variable,
            expression,
            type_annotation,
            span: _,
        } => {
            let ty = synthesize_expression(db, expression, &env, ctx, constraints)?;
            if let Some(annotation) = type_annotation {
                let annotated_ty = resolve(db, annotation, &env, expression.span(), ctx)?;
                if ty != annotated_ty {
                    TypeErrorAccumulator(TypeError::TypeAnnotationMismatch {
                        variable: variable.clone(),
                        annotated: annotated_ty.name(),
                        inferred: ty.name(),
                        span: expression.span(),
                        file_id: ctx.file_id,
                    })
                    .accumulate(db);
                }
            }
            env.declare_variable(variable.clone(), ty, expression.span());
            Some(env)
        }
        Statement::VariableAssignment {
            variable,
            expression,
            span,
        } => {
            let ty = synthesize_expression(db, expression, &env, ctx, constraints)?;
            let (existing_type, _, declaration_span) =
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
        Statement::ExpressionStatement(Expression::Match {
            scrutinee,
            arms,
            span,
        }) => {
            check_match_statement_logic(db, scrutinee, arms, *span, &env, ctx, constraints)?;
            Some(env)
        }
        Statement::ExpressionStatement(expr) => {
            synthesize_expression(db, expr, &env, ctx, constraints)?;
            Some(env)
        }
        Statement::If {
            condition,
            body,
            else_body,
            span: _,
        } => {
            check_boolean_condition(db, condition, &env, ctx, constraints)?;
            check_block(
                db,
                body,
                env.create_child(),
                function_name,
                return_type,
                ctx,
                constraints,
            )?;
            if let Some(else_stmts) = else_body {
                check_block(
                    db,
                    else_stmts,
                    env.create_child(),
                    function_name,
                    return_type,
                    ctx,
                    constraints,
                )?;
            }
            Some(env)
        }
        Statement::IfLet {
            variant_name,
            binding,
            scrutinee,
            body,
            else_body,
            span,
        } => {
            check_if_let_statement(
                db,
                variant_name,
                binding,
                scrutinee,
                body,
                else_body.as_deref(),
                *span,
                &env,
                function_name,
                return_type,
                ctx,
                constraints,
            )?;
            Some(env)
        }
        Statement::While {
            condition,
            body,
            span: _,
        } => {
            check_boolean_condition(db, condition, &env, ctx, constraints)?;
            check_block(
                db,
                body,
                env.create_child(),
                function_name,
                return_type,
                ctx,
                constraints,
            )?;
            Some(env)
        }
        Statement::Return(expr) => {
            let ty = synthesize_expression(db, expr, &env, ctx, constraints)?;
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
        Statement::ForIn {
            variable,
            iterable,
            body,
            ..
        } => {
            let iterable_ty = synthesize_expression(db, iterable, &env, ctx, constraints)?;
            let type_name_str = match &iterable_ty {
                RT::Named(p) | RT::Parameterized(p, _) => p.last_name().to_string(),
                _ => {
                    TypeErrorAccumulator(TypeError::TraitBoundNotSatisfied {
                        type_name: iterable_ty.name(),
                        trait_name: "Iterator".to_string(),
                        param_name: variable.clone(),
                        span: iterable.span(),
                        file_id: ctx.file_id,
                    })
                    .accumulate(db);
                    return None;
                }
            };
            let has_iterator_impl = db.symbol_tables().impls(db).get().values().any(|i| {
                i.type_name.name() == type_name_str
                    && i.trait_name
                        .as_ref()
                        .map(|t| t.name() == "Iterator")
                        .unwrap_or(false)
            });
            if !has_iterator_impl {
                TypeErrorAccumulator(TypeError::TraitBoundNotSatisfied {
                    type_name: type_name_str,
                    trait_name: "Iterator".to_string(),
                    param_name: variable.clone(),
                    span: iterable.span(),
                    file_id: ctx.file_id,
                })
                .accumulate(db);
                return None;
            }
            let mut child_env = env.create_child();
            let element_type = match &iterable_ty {
                RT::Parameterized(_, args) if !args.is_empty() => args[0].clone(),
                _ => RT::Generic("T".to_string()),
            };
            child_env.declare_variable(variable.clone(), element_type, iterable.span());
            check_block(
                db,
                body,
                child_env,
                function_name,
                return_type,
                ctx,
                constraints,
            )?;
            Some(env)
        }
        Statement::Match {
            scrutinee,
            arms,
            span,
        } => {
            check_match_statement_logic(db, scrutinee, arms, *span, &env, ctx, constraints)?;
            Some(env)
        }
    }
}

fn check_if_let_statement(
    db: &dyn TypeCheckDatabase,
    variant_name: &str,
    binding: &str,
    scrutinee: &Expression,
    body: &[Statement],
    else_body: Option<&[Statement]>,
    span: Span,
    env: &TypeEnvironment,
    function_name: &str,
    return_type: &RT,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<()> {
    let scrutinee_type = synthesize_expression(db, scrutinee, env, ctx, constraints)?;
    let variants = match scrutinee_type.union_variants() {
        Some(vs) => vs.to_vec(),
        None => {
            TypeErrorAccumulator(TypeError::MatchOnNonUnion {
                found: scrutinee_type.name(),
                span,
                file_id: ctx.file_id,
            })
            .accumulate(db);
            return None;
        }
    };
    let variant_type = match variants.iter().find(|v| v.name() == variant_name) {
        Some(v) => v.clone(),
        None => {
            TypeErrorAccumulator(TypeError::UnreachableArm {
                variant: variant_name.to_string(),
                span,
                file_id: ctx.file_id,
            })
            .accumulate(db);
            return None;
        }
    };
    let mut child_env = env.create_child();
    child_env.declare_variable(binding.to_string(), variant_type, span);
    check_block(
        db,
        body,
        child_env,
        function_name,
        return_type,
        ctx,
        constraints,
    )?;
    if let Some(else_stmts) = else_body {
        check_block(
            db,
            else_stmts,
            env.create_child(),
            function_name,
            return_type,
            ctx,
            constraints,
        )?;
    }
    Some(())
}

fn check_match_statement_logic(
    db: &dyn TypeCheckDatabase,
    scrutinee: &Expression,
    arms: &[MatchArm],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<()> {
    let scrutinee_type = synthesize_expression(db, scrutinee, env, ctx, constraints)?;
    let variants = match scrutinee_type.union_variants() {
        Some(vs) => vs.to_vec(),
        None => {
            TypeErrorAccumulator(TypeError::MatchOnNonUnion {
                found: scrutinee_type.name(),
                span,
                file_id: ctx.file_id,
            })
            .accumulate(db);
            return None;
        }
    };
    let required_variants: Vec<String> = variants.iter().map(|v| v.name()).collect();
    let mut seen: Vec<String> = Vec::new();
    let mut had_error = false;
    for arm in arms {
        if !required_variants.contains(&arm.variant_name) {
            TypeErrorAccumulator(TypeError::UnreachableArm {
                variant: arm.variant_name.clone(),
                span: arm.span,
                file_id: ctx.file_id,
            })
            .accumulate(db);
            had_error = true;
            continue;
        }
        if seen.contains(&arm.variant_name) {
            TypeErrorAccumulator(TypeError::DuplicateArm {
                variant: arm.variant_name.clone(),
                span: arm.span,
                file_id: ctx.file_id,
            })
            .accumulate(db);
            had_error = true;
            continue;
        }
        seen.push(arm.variant_name.clone());
        let variant_type = variants
            .iter()
            .find(|v| v.name() == arm.variant_name)
            .unwrap()
            .clone();
        let mut child_env = env.create_child();
        child_env.declare_variable(arm.binding.clone(), variant_type, arm.span);
        synthesize_expression(db, &arm.body, &child_env, ctx, constraints)?;
    }
    let missing: Vec<String> = required_variants
        .into_iter()
        .filter(|v| !seen.contains(v))
        .collect();
    if !missing.is_empty() {
        TypeErrorAccumulator(TypeError::IncompleteMatch {
            missing,
            span,
            file_id: ctx.file_id,
        })
        .accumulate(db);
        return None;
    }
    if had_error {
        return None;
    }
    Some(())
}

fn check_expression(
    db: &dyn TypeCheckDatabase,
    expression: &Expression,
    expected: &RT,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<()> {
    match expression {
        Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            check_boolean_condition(db, condition, env, ctx, constraints)?;
            check_expression(db, then_expr, expected, env, ctx, constraints)?;
            check_expression(db, else_expr, expected, env, ctx, constraints)
        }
        Expression::Select(select_expr) => {
            let first = &select_expr.clauses[0];
            let mut all_ok = true;
            for (i, clause) in select_expr.clauses.iter().enumerate() {
                let arm_type =
                    synthesize_expression(db, &clause.expression_to_run, env, ctx, constraints)?;
                if !arm_type.is_assignable_to(expected) {
                    TypeErrorAccumulator(TypeError::SelectBranchTypeMismatch {
                        expected: expected.name(),
                        found: arm_type.name(),
                        branch_index: i,
                        span: clause.expression_to_run.span(),
                        first_branch_span: first.expression_to_run.span(),
                        file_id: ctx.file_id,
                    })
                    .accumulate(db);
                    all_ok = false;
                }
            }
            if all_ok { Some(()) } else { None }
        }
        _ => {
            let got = synthesize_expression(db, expression, env, ctx, constraints)?;
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
    condition: &Expression,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<()> {
    check_expression(db, condition, &RT::boolean(), env, ctx, constraints)
}

fn check_block(
    db: &dyn TypeCheckDatabase,
    stmts: &[Statement],
    mut env: TypeEnvironment,
    function_name: &str,
    return_type: &RT,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<()> {
    for stmt in stmts {
        env = check_statement(db, stmt, env, function_name, return_type, ctx, constraints)?;
    }
    Some(())
}

pub fn resolve_generic_method_sig(
    db: &dyn TypeCheckDatabase,
    param_name: &str,
    method: &str,
    env: &TypeEnvironment,
    span: Span,
    ctx: &CheckContext,
) -> Option<(crate::FunctionSignature, DefinitionPath, String)> {
    let bounds = env.get_type_param_bounds(param_name)?.clone();
    bounds
        .iter()
        .find_map(|bound| resolve_sig_for_bound(db, bound, param_name, method, span, ctx))
}

fn resolve_sig_for_bound(
    db: &dyn TypeCheckDatabase,
    bound: &AstType,
    param_name: &str,
    method: &str,
    span: Span,
    ctx: &CheckContext,
) -> Option<(crate::FunctionSignature, DefinitionPath, String)> {
    let bound_short_name = bound.name().to_string();
    let interned_mod = InternedModuleName::new(db, ctx.module_name.clone());
    let resolved = resolve_type_in_module(db, interned_mod, bound_short_name.clone().intern(db))?;
    let trait_path = resolved.ty;
    let trait_type_def =
        lookup_type_def_in_symbol_tables(db, InternedTypeName::new(db, trait_path.clone()))?;
    let TypeDefinitionKind::Trait { functions, .. } = &trait_type_def.get().kind else {
        return None;
    };
    let entry = functions.iter().find(|e| e.name == method)?;
    let fn_type_def =
        lookup_type_def_in_symbol_tables(db, InternedTypeName::new(db, entry.type_name.clone()))?;
    let TypeDefinitionKind::Function {
        parameters,
        generic_parameters,
        return_type,
    } = &fn_type_def.get().kind
    else {
        return None;
    };
    let type_params_vec: Vec<TypeParam> = generic_parameters
        .iter()
        .map(|gp| TypeParam {
            name: gp.name.clone(),
            bounds: gp.constraints.clone(),
        })
        .collect();
    let type_env = TypeEnvironment::with_type_params(&type_params_vec);
    let mut resolved_params = Vec::new();
    for p in parameters {
        let param_type = if p.type_name.name() == "Self" {
            RT::Generic(param_name.to_string())
        } else {
            resolve(db, &p.type_name, &type_env, span, ctx)?
        };
        resolved_params.push(crate::typed_ast::Parameter {
            name: p.name.clone(),
            param_type,
            binding_id: BindingId(0),
            span,
        });
    }
    let resolved_return = if return_type.name() == "Self" {
        RT::Generic(param_name.to_string())
    } else {
        resolve(db, return_type, &type_env, span, ctx)?
    };
    Some((
        crate::FunctionSignature {
            parameters: resolved_params,
            return_type: resolved_return,
            type_params: type_params_vec,
            kind: FunctionKind::Bytecode,
        },
        trait_path,
        bound_short_name,
    ))
}

fn resolve_method_sig(
    db: &dyn TypeCheckDatabase,
    receiver_type: &RT,
    method: &str,
    env: &TypeEnvironment,
    span: Span,
    ctx: &CheckContext,
) -> Option<crate::FunctionSignature> {
    if receiver_type.is_actor_ref() {
        let module_type = receiver_type.actor_ref_inner()?;
        let module_path = match module_type {
            RT::Named(p) | RT::Parameterized(p, _) => p.clone(),
            _ => return None,
        };
        let fn_path = DefinitionPath::for_function(module_path, method);
        return get_function_sig(db, InternedFunctionName::new(db, fn_path), ctx.program)
            .map(|s| s.get().clone());
    }
    if let RT::Generic(param_name) = receiver_type {
        let (sig, _, _) = resolve_generic_method_sig(db, param_name, method, env, span, ctx)?;
        return Some(sig);
    }
    let type_path = match receiver_type {
        RT::Named(tn) | RT::Parameterized(tn, _) => tn,
        _ => return None,
    };
    let impl_fn_path = find_impl_fn_by_type_path(db, type_path, method).or_accumulate(
        db,
        TypeError::UnknownFunction {
            name: method.to_string(),
            span,
            file_id: ctx.file_id,
        },
    )?;
    get_function_sig(db, InternedFunctionName::new(db, impl_fn_path), ctx.program)
        .map(|s| s.get().clone())
}

pub fn synthesize_expression(
    db: &dyn TypeCheckDatabase,
    expression: &Expression,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<RT> {
    match expression {
        Expression::Call {
            function,
            type_args,
            arguments,
            span,
        } => synthesize_call(
            db,
            function,
            type_args,
            arguments,
            *span,
            env,
            ctx,
            constraints,
        ),
        Expression::Variable { name, span } => {
            env.lookup_variable(name).map(|(ty, _)| ty).or_accumulate(
                db,
                TypeError::UnknownVariable {
                    name: name.clone(),
                    span: *span,
                    file_id: ctx.file_id,
                },
            )
        }
        Expression::StringLiteral { .. } => Some(RT::string()),
        Expression::BooleanLiteral { .. } => Some(RT::boolean()),
        Expression::IntLiteral { .. } => Some(RT::int()),
        Expression::UnitLiteral { .. } => Some(RT::unit()),
        Expression::Placeholder { span } => {
            TypeErrorAccumulator(TypeError::TypeMismatch {
                expected: "concrete type".to_string(),
                found: "placeholder".to_string(),
                span: *span,
                file_id: ctx.file_id,
            })
            .accumulate(db);
            None
        }
        Expression::ListLiteral { elements, span } => {
            synthesize_list_literal(db, elements, *span, env, ctx, constraints)
        }
        Expression::Select(select_expr) => synthesize_select(
            db,
            &select_expr.clauses,
            select_expr.span,
            env,
            ctx,
            constraints,
        ),
        Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            span,
        } => synthesize_if_else(
            db,
            condition,
            then_expr,
            else_expr,
            *span,
            env,
            ctx,
            constraints,
        ),
        Expression::StructLiteral {
            struct_name,
            fields,
            span,
        } => synthesize_struct_literal(db, struct_name, fields, *span, env, ctx, constraints),
        Expression::FieldAccess { base, field, span } => {
            synthesize_field_access(db, base, field, *span, env, ctx, constraints)
        }
        Expression::Spawn { type_arg, span, .. } => {
            let module_type = resolve(db, type_arg, env, *span, ctx)?;
            Some(RT::actor_ref(module_type))
        }
        Expression::MethodCall {
            receiver,
            method,
            args,
            span,
        } => {
            let receiver_type = synthesize_expression(db, receiver, env, ctx, constraints)?;
            let sig = resolve_method_sig(db, &receiver_type, method, env, *span, ctx)?;
            let mut subst = Substitution::new();
            let mut param_names: HashMap<String, String> = HashMap::new();
            let param_offset = if receiver_type.is_actor_ref() {
                0
            } else {
                if let Some(first_param) = sig.parameters.first() {
                    if let Some(bindings) =
                        extract_generic_bindings(&first_param.param_type, &receiver_type)
                    {
                        for name in bindings.keys() {
                            param_names
                                .entry(name.clone())
                                .or_insert(first_param.name.clone());
                        }
                        subst.extend(bindings);
                    }
                }
                1
            };
            for (arg, param) in args.iter().zip(sig.parameters.iter().skip(param_offset)) {
                if matches!(arg, Expression::Placeholder { .. }) {
                    continue;
                }
                let arg_ty = synthesize_expression(db, arg, env, ctx, constraints)?;
                if let Some(bindings) = extract_generic_bindings(&param.param_type, &arg_ty) {
                    for name in bindings.keys() {
                        param_names
                            .entry(name.clone())
                            .or_insert(param.name.clone());
                    }
                    subst.extend(bindings);
                }
            }
            for tp in &sig.type_params {
                if let Some(resolved_ty) = subst.get(&tp.name) {
                    constraints.push(Constraint {
                        kind: ConstraintKind::Unify {
                            call_site: span.start,
                            var: tp.name.clone(),
                            ty: resolved_ty.clone(),
                            fn_name: method.clone(),
                            param_name: param_names.get(&tp.name).cloned().unwrap_or_default(),
                        },
                        span: *span,
                        file_id: ctx.file_id,
                        flavour: Flavour::Wanted,
                    });
                }
            }
            Some(subst.apply_subst(&sig.return_type))
        }
        Expression::StringTemplate { parts, .. } => {
            for part in parts {
                if let StringPart::Interpolated(expr) = part {
                    synthesize_expression(db, expr, env, ctx, constraints);
                }
            }
            Some(RT::string())
        }
        Expression::Match {
            scrutinee,
            arms,
            span,
        } => {
            let scrutinee_type = synthesize_expression(db, scrutinee, env, ctx, constraints)?;
            let variants = match scrutinee_type.union_variants() {
                Some(vs) => vs.to_vec(),
                None => {
                    TypeErrorAccumulator(TypeError::MatchOnNonUnion {
                        found: scrutinee_type.name(),
                        span: *span,
                        file_id: ctx.file_id,
                    })
                    .accumulate(db);
                    return None;
                }
            };
            let required_variants: Vec<String> = variants.iter().map(|v| v.name()).collect();
            let mut seen: Vec<String> = Vec::new();
            let mut had_error = false;
            let mut first_arm_type: Option<RT> = None;
            for arm in arms {
                if !required_variants.contains(&arm.variant_name) {
                    TypeErrorAccumulator(TypeError::UnreachableArm {
                        variant: arm.variant_name.clone(),
                        span: arm.span,
                        file_id: ctx.file_id,
                    })
                    .accumulate(db);
                    had_error = true;
                    continue;
                }
                if seen.contains(&arm.variant_name) {
                    TypeErrorAccumulator(TypeError::DuplicateArm {
                        variant: arm.variant_name.clone(),
                        span: arm.span,
                        file_id: ctx.file_id,
                    })
                    .accumulate(db);
                    had_error = true;
                    continue;
                }
                seen.push(arm.variant_name.clone());
                let variant_type = variants
                    .iter()
                    .find(|v| v.name() == arm.variant_name)
                    .unwrap()
                    .clone();
                let mut child_env = env.create_child();
                child_env.declare_variable(arm.binding.clone(), variant_type, arm.span);
                let arm_ty = synthesize_expression(db, &arm.body, &child_env, ctx, constraints)?;
                match &first_arm_type {
                    None => first_arm_type = Some(arm_ty),
                    Some(expected) => {
                        if arm_ty != *expected {
                            TypeErrorAccumulator(TypeError::TypeMismatch {
                                expected: expected.name(),
                                found: arm_ty.name(),
                                span: arm.span,
                                file_id: ctx.file_id,
                            })
                            .accumulate(db);
                            had_error = true;
                        }
                    }
                }
            }
            let missing: Vec<String> = required_variants
                .into_iter()
                .filter(|v| !seen.contains(v))
                .collect();
            if !missing.is_empty() {
                TypeErrorAccumulator(TypeError::IncompleteMatch {
                    missing,
                    span: *span,
                    file_id: ctx.file_id,
                })
                .accumulate(db);
                return None;
            }
            if had_error {
                return None;
            }
            first_arm_type
        }
    }
}

fn synthesize_call(
    db: &dyn TypeCheckDatabase,
    function: &str,
    type_args: &[AstType],
    arguments: &[Expression],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<RT> {
    let interned_current = InternedModuleName::new(db, ctx.module_name.clone());
    let interned_fn = function.intern(db);

    let (resolved_fn_name, sig) = resolve_function_call(db, interned_current, interned_fn)
        .and_then(|fn_name| {
            let interned = InternedFunctionName::new(db, fn_name.clone());
            get_function_sig(db, interned, ctx.program).map(|arc| (fn_name, arc.get().clone()))
        })
        .or_accumulate(
            db,
            TypeError::UnknownFunction {
                name: function.to_string(),
                span,
                file_id: ctx.file_id,
            },
        )?;

    check_visibility(db, &resolved_fn_name, span, ctx)?;

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

    let mut subst = Substitution::new();

    for (tp, ty_arg) in sig.type_params.iter().zip(type_args) {
        if let Some(resolved) = resolve(db, ty_arg, env, span, ctx) {
            subst.bind(tp.name.clone(), resolved.clone());
            constraints.push(Constraint {
                kind: ConstraintKind::Unify {
                    call_site: span.start,
                    var: tp.name.clone(),
                    ty: resolved,
                    fn_name: function.to_string(),
                    param_name: String::new(),
                },
                span,
                file_id: ctx.file_id,
                flavour: Flavour::Wanted,
            });
        }
    }

    for (arg, param) in arguments.iter().zip(&sig.parameters) {
        if matches!(arg, Expression::Placeholder { .. }) {
            continue;
        }
        let arg_ty = synthesize_expression(db, arg, env, ctx, constraints)?;
        let new_bindings = extract_generic_bindings(&param.param_type, &arg_ty);
        ensure_or_accumulate!(
            new_bindings.is_some(),
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
        let new_bindings = new_bindings.unwrap();
        for (name, ty) in &new_bindings {
            if sig.type_params.iter().any(|tp| &tp.name == name) {
                constraints.push(Constraint {
                    kind: ConstraintKind::Unify {
                        call_site: span.start,
                        var: name.clone(),
                        ty: ty.clone(),
                        fn_name: function.to_string(),
                        param_name: param.name.clone(),
                    },
                    span,
                    file_id: ctx.file_id,
                    flavour: Flavour::Wanted,
                });
            }
        }
        subst.extend(new_bindings);
    }
    for tp in &sig.type_params {
        if tp.bounds.is_empty() {
            continue;
        }
        if let Some(actual) = subst.get(&tp.name) {
            let interned_mod = InternedModuleName::new(db, ctx.module_name.clone());
            let type_path = match actual {
                RT::Named(path) => Some(path.clone()),
                RT::Parameterized(path, _) => Some(path.clone()),
                RT::Generic(_) => None,
                RT::Union(_) => None,
            };
            for bound in &tp.bounds {
                let bound_name = bound.name().to_string();
                let bound_path =
                    resolve_type_in_module(db, interned_mod, bound_name.clone().intern(db))
                        .map(|r| r.ty);
                let types_table = db.symbol_tables().types(db);
                let is_trait = bound_path
                    .as_ref()
                    .and_then(|p| types_table.get().get(p))
                    .map(|td| matches!(&td.kind, TypeDefinitionKind::Trait { .. }))
                    .unwrap_or(false);
                if is_trait {
                    if let (Some(type_path), Some(trait_path)) = (type_path.clone(), bound_path) {
                        constraints.push(Constraint {
                            kind: ConstraintKind::TraitBound {
                                type_path,
                                trait_path,
                                type_name: actual.name().to_string(),
                                trait_name: bound_name,
                                param_name: tp.name.clone(),
                            },
                            span,
                            file_id: ctx.file_id,
                            flavour: Flavour::Wanted,
                        });
                    }
                } else if let Some(bound_type) = resolve(db, bound, env, span, ctx) {
                    constraints.push(Constraint {
                        kind: ConstraintKind::TypeBound {
                            caller: ctx.module_name.to_string(),
                            callee: function.to_string(),
                            actual_type: actual.clone(),
                            bound_type,
                            context: format!(
                                "call to '{}': type parameter '{}' bound",
                                function, tp.name
                            ),
                        },
                        span,
                        file_id: ctx.file_id,
                        flavour: Flavour::Wanted,
                    });
                }
            }
        }
    }
    Some(subst.apply_subst(&sig.return_type))
}

fn synthesize_list_literal(
    db: &dyn TypeCheckDatabase,
    elements: &[Expression],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
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
    let first_type = synthesize_expression(db, &elements[0], env, ctx, constraints)?;
    for elem in elements.iter().skip(1) {
        check_expression(db, elem, &first_type, env, ctx, constraints)?;
    }
    Some(RT::list(first_type))
}

fn synthesize_select(
    db: &dyn TypeCheckDatabase,
    clauses: &[SelectClause],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
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
    let mut arm_types = Vec::with_capacity(clauses.len());
    for clause in clauses {
        let arm_type = synthesize_expression(db, &clause.expression_to_run, env, ctx, constraints)?;
        arm_types.push(arm_type);
    }
    Some(RT::union(arm_types))
}

fn synthesize_if_else(
    db: &dyn TypeCheckDatabase,
    condition: &Expression,
    then_expr: &Expression,
    else_expr: &Expression,
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<RT> {
    let _ = span;
    check_boolean_condition(db, condition, env, ctx, constraints)?;
    let then_type = synthesize_expression(db, then_expr, env, ctx, constraints)?;
    check_expression(db, else_expr, &then_type, env, ctx, constraints)?;
    Some(then_type)
}

fn synthesize_struct_literal(
    db: &dyn TypeCheckDatabase,
    struct_name: &str,
    fields: &[(String, Expression)],
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<RT> {
    let (definition, type_params) = get_struct_fields(db, struct_name, ctx.module_name)
        .or_accumulate(
            db,
            TypeError::UnsupportedType {
                type_name: struct_name.to_string(),
                span,
                file_id: ctx.file_id,
            },
        )?;
    let mut seen = std::collections::HashSet::new();
    let mut subst = Substitution::new();
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
        let declared_type = resolve(db, &declared_ast_type, &type_env, value_expr.span(), ctx)?;
        let value_type = synthesize_expression(db, value_expr, env, ctx, constraints)?;
        ensure_or_accumulate!(
            subst.match_type(&declared_type, &value_type).is_ok(),
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
        let interned_mod = InternedModuleName::new(db, ctx.module_name.clone());
        let interned_name = struct_name.intern(db);
        resolve_type_in_module(db, interned_mod, interned_name)
            .map(|r| r.ty)
            .unwrap_or_else(|| DefinitionPath::for_type(ctx.module_name.clone(), struct_name))
    };
    if type_params.is_empty() {
        Some(RT::Named(resolved_type_name))
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
        Some(RT::Parameterized(resolved_type_name, args))
    }
}

fn synthesize_field_access(
    db: &dyn TypeCheckDatabase,
    base: &Expression,
    field: &str,
    span: Span,
    env: &TypeEnvironment,
    ctx: &CheckContext,
    constraints: &mut Vec<Constraint>,
) -> Option<RT> {
    let base_type = synthesize_expression(db, base, env, ctx, constraints)?;
    match base_type {
        RT::Named(type_name) => {
            let (definition, type_params) =
                get_struct_fields(db, type_name.last_name(), ctx.module_name).or_accumulate(
                    db,
                    TypeError::UnsupportedType {
                        type_name: type_name.last_name().to_string(),
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
                        struct_name: type_name.last_name().to_string(),
                        field_name: field.to_string(),
                        span,
                        file_id: ctx.file_id,
                    },
                )?;
            resolve(db, &field_ast_type, &type_env, span, ctx)
        }
        RT::Generic(name) => {
            let (definition, type_params) = get_struct_fields(db, &name, ctx.module_name)
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
            resolve(db, &field_ast_type, &type_env, span, ctx)
        }
        other => {
            TypeErrorAccumulator(TypeError::TypeMismatch {
                expected: "struct".to_string(),
                found: other.name(),
                span,
                file_id: ctx.file_id,
            })
            .accumulate(db);
            None
        }
    }
}

fn check_visibility(
    db: &dyn TypeCheckDatabase,
    fn_name: &DefinitionPath,
    span: Span,
    ctx: &CheckContext,
) -> Option<()> {
    if &fn_name.module_prefix() == ctx.module_name {
        return Some(());
    }
    let interned = InternedFunctionName::new(db, fn_name.clone());
    let is_visible = lookup_function_def(db, interned)
        .map(|arc_ptr| matches!(arc_ptr.get().visibility, Visibility::Public))
        .unwrap_or(true);
    ensure_or_accumulate!(
        is_visible,
        db,
        TypeError::PrivateFunction {
            name: format!("{}::{}", fn_name.module_prefix(), fn_name.last_name()),
            span,
            file_id: ctx.file_id,
        }
    );
    Some(())
}
