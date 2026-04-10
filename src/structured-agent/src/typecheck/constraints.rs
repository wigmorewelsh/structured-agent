use super::TypeChecker;
use super::db::{
    InternedModuleName, InternedString, SymbolTablesInput, TypeCheckDatabase, resolve_type_alias,
};
use super::refs::CheckerAstRef;
use crate::ast::{Type as AstType, TypeParam};
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span};
use std::collections::HashMap;
use structured_agent_runtime::symbols::{ModuleName, TypeName};

pub(super) fn resolve_type(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    t: &AstType,
    module: &ModuleName,
) -> AstType {
    match t {
        AstType::Generic(name)
            if {
                let interned_mod = InternedModuleName::new(db, module.clone());
                let interned_name = InternedString::new(db, name.clone());
                let resolved = resolve_type_alias(db, tables, interned_mod, interned_name)
                    .unwrap_or_else(|| TypeName {
                        name: name.clone(),
                        module: module.clone(),
                    });
                tables
                    .types(db)
                    .get()
                    .get(&resolved)
                    .map(|td| matches!(td.ast_ref, CheckerAstRef::Struct(_)))
                    .unwrap_or(false)
            } =>
        {
            AstType::Struct(name.clone())
        }
        AstType::List(inner) => AstType::List(Box::new(resolve_type(db, tables, inner, module))),
        AstType::Option(inner) => {
            AstType::Option(Box::new(resolve_type(db, tables, inner, module)))
        }
        other => other.clone(),
    }
}

pub(super) fn validate_type_with_params(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    ast_type: &AstType,
    span: Span,
    file_id: FileId,
    type_params: &[TypeParam],
    module: &ModuleName,
) -> Result<(), TypeError> {
    match ast_type {
        AstType::Unit | AstType::Boolean | AstType::String | AstType::Int => Ok(()),
        AstType::Generic(name) => {
            if name == "Self" || type_params.iter().any(|tp| tp.name == *name) {
                Ok(())
            } else {
                Err(TypeError::UnboundTypeParameter {
                    name: name.clone(),
                    span,
                    file_id,
                })
            }
        }
        AstType::List(inner) | AstType::Option(inner) => {
            validate_type_with_params(db, tables, inner, span, file_id, type_params, module)
        }
        AstType::Struct(name) => {
            let interned_mod = InternedModuleName::new(db, module.clone());
            let interned_name = InternedString::new(db, name.clone());
            let resolved = resolve_type_alias(db, tables, interned_mod, interned_name)
                .unwrap_or_else(|| TypeName {
                    name: name.clone(),
                    module: module.clone(),
                });
            if tables
                .types(db)
                .get()
                .get(&resolved)
                .map(|td| matches!(td.ast_ref, CheckerAstRef::Struct(_)))
                .unwrap_or(false)
            {
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

impl TypeChecker {
    pub(super) fn unify_type(
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

    pub(super) fn apply_subst(ty: &AstType, subst: &HashMap<String, AstType>) -> AstType {
        match ty {
            AstType::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            AstType::List(inner) => AstType::List(Box::new(Self::apply_subst(inner, subst))),
            AstType::Option(inner) => AstType::Option(Box::new(Self::apply_subst(inner, subst))),
            other => other.clone(),
        }
    }
}
