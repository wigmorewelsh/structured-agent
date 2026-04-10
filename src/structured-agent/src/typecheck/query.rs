use super::db::{
    InternedFunctionName, InternedModuleName, InternedString, InternedTraitName, InternedTypeName,
    SymbolTablesInput, TypeCheckDatabase, find_trait_for_impl_call, lookup_function_def,
    lookup_impl_exists, lookup_trait_def, lookup_type_def, resolve_type_alias,
};
use super::refs::{CheckerAstRef, CheckerRefs};
use super::{CheckContext, FunctionSignature, TypeChecker, TypeEnvironment};
use crate::ast::{Expression, SigFunction, Type as AstType};
use crate::typecheck::error::TypeError;
use crate::types::Span;
use nonempty::NonEmpty;
use structured_agent_runtime::symbols::{
    FunctionDefinition, FunctionName, FunctionNameKind, ModuleName, TypeName, Visibility,
};

pub(super) fn get_function_sig(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &FunctionName,
) -> Option<FunctionSignature> {
    let make_sig = |f: &FunctionDefinition<CheckerRefs>| match &f.ast_ref {
        CheckerAstRef::Function(func, kind) => Some(FunctionSignature {
            parameters: func
                .parameters
                .iter()
                .map(|p| crate::ast::Parameter {
                    name: p.name.clone(),
                    param_type: super::constraints::resolve_type(
                        db,
                        tables,
                        &p.param_type,
                        &name.module,
                    ),
                    span: p.span,
                })
                .collect(),
            return_type: super::constraints::resolve_type(
                db,
                tables,
                &func.return_type,
                &name.module,
            ),
            type_params: func.type_params.clone(),
            kind: kind.clone(),
        }),
        CheckerAstRef::ImplFunction(func, concrete_type, kind) => Some(FunctionSignature {
            parameters: func
                .parameters
                .iter()
                .map(|p| crate::ast::Parameter {
                    name: p.name.clone(),
                    param_type: super::constraints::resolve_type(
                        db,
                        tables,
                        &TypeChecker::substitute_self(&p.param_type, concrete_type),
                        &name.module,
                    ),
                    span: p.span,
                })
                .collect(),
            return_type: super::constraints::resolve_type(
                db,
                tables,
                &TypeChecker::substitute_self(&func.return_type, concrete_type),
                &name.module,
            ),
            type_params: func.type_params.clone(),
            kind: kind.clone(),
        }),
        CheckerAstRef::ExternalFn {
            params,
            return_type,
            type_params,
            kind,
        } => Some(FunctionSignature {
            parameters: params.clone(),
            return_type: return_type.clone(),
            type_params: type_params.clone(),
            kind: kind.clone(),
        }),
        _ => None,
    };
    let key = InternedFunctionName::new(db, name.clone());
    lookup_function_def(db, tables, key).and_then(|arc_ptr| make_sig(arc_ptr.get()))
}

pub(super) fn get_struct_fields(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &str,
    current_module: &ModuleName,
) -> Option<Vec<(String, AstType)>> {
    let interned_mod = InternedModuleName::new(db, current_module.clone());
    let interned_name = InternedString::new(db, name.to_string());
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
    let interned_mod = InternedModuleName::new(db, current_module.clone());
    let interned_name = InternedString::new(db, name.to_string());
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
        AstType::Int => "Int".to_string(),
        AstType::String => "String".to_string(),
        AstType::Boolean => "Boolean".to_string(),
        AstType::Struct(n) => n.clone(),
        _ => return None,
    };
    let interned_fn = InternedString::new(db, fn_name.to_string());
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
    let interned = InternedFunctionName::new(db, fn_name.clone());
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
