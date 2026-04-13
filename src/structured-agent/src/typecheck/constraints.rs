use super::TypeChecker;
use super::db::{Intern, SymbolTablesInput, TypeCheckDatabase, resolve_type_alias};
use super::refs::CheckerAstRef;
use crate::ast::{Type as AstType, TypeParam};
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span};
use std::collections::HashMap;
use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::{ModuleName, TypeName};

pub(super) fn resolve(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    t: &AstType,
    module: &ModuleName,
    type_params: &[TypeParam],
    span: Span,
    file_id: FileId,
) -> Result<RT, TypeError> {
    match t {
        AstType::Generic(name) | AstType::Struct(name) => {
            match name.as_str() {
                "Unit" => return Ok(RT::Unit),
                "Boolean" => return Ok(RT::Boolean),
                "String" => return Ok(RT::String),
                "Int" => return Ok(RT::Int),
                _ => {}
            }
            if name == "Self" || type_params.iter().any(|tp| tp.name == *name) {
                return Ok(RT::Generic(name.clone()));
            }
            let interned_mod = module.intern(db);
            let interned_name = name.intern(db);
            let type_name = resolve_type_alias(db, tables, interned_mod, interned_name)
                .unwrap_or_else(|| TypeName {
                    name: name.clone(),
                    module: module.clone(),
                });
            match tables.types(db).get().get(&type_name).map(|td| &td.ast_ref) {
                Some(CheckerAstRef::Struct(_)) => Ok(RT::Struct(type_name)),
                _ => Err(TypeError::UnboundTypeParameter {
                    name: name.clone(),
                    span,
                    file_id,
                }),
            }
        }
        AstType::List(inner) => Ok(RT::List(Box::new(resolve(
            db,
            tables,
            inner,
            module,
            type_params,
            span,
            file_id,
        )?))),
        AstType::Option(inner) => Ok(RT::Option(Box::new(resolve(
            db,
            tables,
            inner,
            module,
            type_params,
            span,
            file_id,
        )?))),
    }
}

impl TypeChecker {
    pub(super) fn unify_type(formal: &RT, actual: &RT, subst: &mut HashMap<String, RT>) -> bool {
        match formal {
            RT::Generic(name) => {
                if let Some(bound) = subst.get(name) {
                    bound == actual
                } else {
                    subst.insert(name.clone(), actual.clone());
                    true
                }
            }
            RT::List(inner_formal) => {
                if let RT::List(inner_actual) = actual {
                    Self::unify_type(inner_formal, inner_actual, subst)
                } else {
                    false
                }
            }
            RT::Option(inner_formal) => {
                if let RT::Option(inner_actual) = actual {
                    Self::unify_type(inner_formal, inner_actual, subst)
                } else {
                    false
                }
            }
            _ => formal == actual,
        }
    }

    pub(super) fn apply_subst(ty: &RT, subst: &HashMap<String, RT>) -> RT {
        match ty {
            RT::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            RT::List(inner) => RT::List(Box::new(Self::apply_subst(inner, subst))),
            RT::Option(inner) => RT::Option(Box::new(Self::apply_subst(inner, subst))),
            other => other.clone(),
        }
    }
}
