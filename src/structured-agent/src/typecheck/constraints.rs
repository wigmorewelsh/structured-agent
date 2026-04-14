use super::TypeChecker;
use super::db::{Intern, SymbolTablesInput, TypeCheckDatabase, resolve_type_alias};

use crate::ast::{Type as AstType, TypeParam};
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span};
use nonempty::NonEmpty;
use std::collections::HashMap;
use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::{ModuleName, TypeDefinitionKind, TypeName};

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
            match tables.types(db).get().get(&type_name) {
                Some(td) => match &td.kind {
                    TypeDefinitionKind::Struct { .. } => Ok(RT::Struct(type_name)),
                    _ => Err(TypeError::UnboundTypeParameter {
                        name: name.clone(),
                        span,
                        file_id,
                    }),
                },
                None => Err(TypeError::UnboundTypeParameter {
                    name: name.clone(),
                    span,
                    file_id,
                }),
            }
        }
        AstType::Parameterized(name, inner) => {
            let interned_mod = ModuleName::new(NonEmpty::new("prelude".to_string())).intern(db);
            let interned_name = name.intern(db);
            let type_name = resolve_type_alias(db, tables, interned_mod, interned_name)
                .unwrap_or_else(|| TypeName {
                    name: name.clone(),
                    module: ModuleName::new(NonEmpty::new("prelude".to_string())),
                });
            match tables.types(db).get().get(&type_name) {
                Some(td) => match &td.kind {
                    TypeDefinitionKind::Native { .. } => {
                        let inner_rt =
                            resolve(db, tables, inner, module, type_params, span, file_id)?;
                        Ok(RT::Parameterized(type_name, vec![inner_rt]))
                    }
                    _ => Err(TypeError::UnboundTypeParameter {
                        name: name.clone(),
                        span,
                        file_id,
                    }),
                },
                None => Err(TypeError::UnboundTypeParameter {
                    name: name.clone(),
                    span,
                    file_id,
                }),
            }
        }
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
            RT::Parameterized(name_formal, args_formal) => {
                if let RT::Parameterized(name_actual, args_actual) = actual {
                    name_formal == name_actual
                        && args_formal.len() == args_actual.len()
                        && args_formal
                            .iter()
                            .zip(args_actual.iter())
                            .all(|(f, a)| Self::unify_type(f, a, subst))
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
            RT::Parameterized(name, args) => RT::Parameterized(
                name.clone(),
                args.iter().map(|a| Self::apply_subst(a, subst)).collect(),
            ),
            other => other.clone(),
        }
    }
}
