use super::db::{
    Intern, InternedTraitName, InternedTypeName, SymbolTablesInput, TypeCheckDatabase,
    find_trait_for_impl_call, lookup_function_def, lookup_impl_exists, lookup_trait_def,
    lookup_type_def, resolve_type_alias,
};
use super::refs::{CheckerAstRef, FunctionKind};
use super::{CheckContext, FunctionSignature, TypeChecker, TypeEnvironment};
use crate::ast::{Expression, SigFunction, Type as AstType, TypeParam};
use crate::typecheck::error::TypeError;
use crate::types::Span;
use nonempty::NonEmpty;
use structured_agent_runtime::symbols::{
    FunctionName, FunctionNameKind, ModuleName, TypeDefinitionKind, TypeName, Visibility,
};

pub(super) fn get_function_sig(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &FunctionName,
) -> Option<FunctionSignature> {
    let key = name.intern(db);
    let fn_def = lookup_function_def(db, tables, key)?;

    let kind = match &fn_def.get().ast_ref {
        CheckerAstRef::ExternalFn { .. } => FunctionKind::External,
        _ => FunctionKind::Bytecode,
    };

    let concrete_type = match &name.kind {
        FunctionNameKind::Impl { type_name, .. } => Some(type_name.clone()),
        _ => None,
    };

    let type_key = InternedTypeName::new(db, fn_def.get().type_name.clone());
    let type_def = lookup_type_def(db, tables, type_key)?;

    let TypeDefinitionKind::Function {
        parameters,
        generic_parameters,
        return_type,
    } = &type_def.get().kind
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

    let mut resolved_params = Vec::with_capacity(parameters.len());
    for p in parameters {
        let substituted = match &concrete_type {
            Some(ct) => TypeChecker::substitute_self(&p.type_name, ct),
            None => p.type_name.clone(),
        };
        let param_type = super::constraints::resolve(
            db,
            tables,
            &substituted,
            &name.module,
            &type_params_vec,
            Span::dummy(),
            0,
        )
        .ok()?;
        resolved_params.push(crate::typed_ast::Parameter {
            name: p.name.clone(),
            param_type,
            span: Span::dummy(),
        });
    }

    let subst_return = match &concrete_type {
        Some(ct) => TypeChecker::substitute_self(return_type, ct),
        None => return_type.clone(),
    };
    let resolved_return = super::constraints::resolve(
        db,
        tables,
        &subst_return,
        &name.module,
        &type_params_vec,
        Span::dummy(),
        0,
    )
    .ok()?;

    Some(FunctionSignature {
        parameters: resolved_params,
        return_type: resolved_return,
        type_params: type_params_vec,
        kind,
    })
}

pub(super) fn get_struct_fields(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &str,
    current_module: &ModuleName,
) -> Option<Vec<(String, AstType)>> {
    let interned_mod = current_module.intern(db);
    let interned_name = name.intern(db);
    let resolved =
        resolve_type_alias(db, tables, interned_mod, interned_name).unwrap_or_else(|| TypeName {
            name: name.to_string(),
            module: current_module.clone(),
        });
    let extract = |ast_ref: &CheckerAstRef| {
        if let CheckerAstRef::Struct(s) = ast_ref {
            Some(
                s.fields
                    .iter()
                    .map(|f| (f.name.clone(), f.field_type.clone()))
                    .collect(),
            )
        } else {
            None
        }
    };
    let key = InternedTypeName::new(db, resolved);
    lookup_type_def(db, tables, key).and_then(|arc_ptr| extract(&arc_ptr.get().ast_ref))
}

pub(super) fn get_trait_functions(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &str,
    current_module: &ModuleName,
) -> Option<Vec<SigFunction>> {
    let interned_mod = current_module.intern(db);
    let interned_name = name.intern(db);
    let resolved =
        resolve_type_alias(db, tables, interned_mod, interned_name).unwrap_or_else(|| TypeName {
            name: name.to_string(),
            module: current_module.clone(),
        });
    let trait_name = TypeName {
        name: resolved.name,
        module: resolved.module,
    };
    let key = InternedTraitName::new(db, trait_name);
    lookup_trait_def(db, tables, key).and_then(|arc_ptr| {
        if let CheckerAstRef::Trait(t) = &arc_ptr.get().ast_ref {
            Some(t.functions.clone())
        } else {
            None
        }
    })
}

pub(super) fn type_implements_trait(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    type_name: &str,
    trait_name: &str,
) -> bool {
    let tn = TypeName {
        name: type_name.to_string(),
        module: ModuleName::new(NonEmpty::new(String::new())),
    };
    let trn = TypeName {
        name: trait_name.to_string(),
        module: ModuleName::new(NonEmpty::new(String::new())),
    };
    let type_key = InternedTypeName::new(db, tn);
    let trait_key = InternedTraitName::new(db, trn);
    lookup_impl_exists(db, tables, type_key, trait_key)
}

pub(super) fn resolve_impl_call(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    fn_name: &str,
    arguments: &[Expression],
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<(FunctionName, FunctionSignature)> {
    if arguments.is_empty() {
        return None;
    }
    let first_arg =
        super::elaboration::check_expression(db, tables, &arguments[0], env, ctx).ok()?;
    let type_name = match first_arg.ty() {
        structured_agent_runtime::Type::Int => "Int".to_string(),
        structured_agent_runtime::Type::String => "String".to_string(),
        structured_agent_runtime::Type::Boolean => "Boolean".to_string(),
        structured_agent_runtime::Type::Struct(tn) => tn.name.clone(),
        _ => return None,
    };
    let interned_fn = fn_name.intern(db);
    let interned_type = InternedTypeName::new(
        db,
        TypeName {
            name: type_name.clone(),
            module: ModuleName::new(NonEmpty::new(String::new())),
        },
    );
    if let Some(interned_trait) = find_trait_for_impl_call(db, tables, interned_fn, interned_type) {
        let trait_key_name = interned_trait.name(db);
        let impl_fn_name = {
            let mn = ctx.module_name.clone();
            FunctionName {
                name: fn_name.to_string(),
                module: mn.clone(),
                kind: FunctionNameKind::Impl {
                    type_name: type_name.to_string(),
                    trait_name: trait_key_name.name.to_string(),
                },
            }
        };
        if let Some(sig) = get_function_sig(db, tables, &impl_fn_name) {
            return Some((impl_fn_name, sig));
        }
    }
    None
}

pub(super) fn check_visibility(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    fn_name: &FunctionName,
    span: Span,
    ctx: &CheckContext,
) -> Result<(), TypeError> {
    if &fn_name.module == ctx.module_name {
        return Ok(());
    }
    let interned = fn_name.intern(db);
    let is_visible = lookup_function_def(db, tables, interned)
        .map(|arc_ptr| matches!(arc_ptr.get().visibility, Visibility::Public))
        .unwrap_or(true);

    if is_visible {
        Ok(())
    } else {
        Err(TypeError::PrivateFunction {
            name: format!("{}::{}", fn_name.module, fn_name.name),
            span,
            file_id: ctx.file_id,
        })
    }
}
