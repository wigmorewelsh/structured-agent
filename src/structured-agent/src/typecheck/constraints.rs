use super::db::{Intern, SymbolTablesInput, TypeCheckDatabase, resolve_type_alias};

use crate::ast::{Type as AstType, TypeParam};
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span};

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
) -> Option<RT> {
    let AstType { name, args } = t;

    if name == "Self" || type_params.iter().any(|tp| &tp.name == name) {
        return Some(RT::Generic(name.to_string()));
    }

    let interned_mod = module.intern(db);
    let interned_name = name.intern(db);
    let type_name =
        resolve_type_alias(db, tables, interned_mod, interned_name).unwrap_or_else(|| TypeName {
            name: name.clone(),
            module: module.clone(),
        });

    match tables.types(db).get().get(&type_name) {
        Some(td) => match &td.kind {
            TypeDefinitionKind::Struct { .. } | TypeDefinitionKind::Primitive
                if args.is_empty() =>
            {
                Some(RT::Struct(type_name))
            }
            TypeDefinitionKind::Native { .. } => {
                let inner_rt = resolve(db, tables, &args[0], module, type_params, span, file_id)?;
                Some(RT::Parameterized(type_name, vec![inner_rt]))
            }
            TypeDefinitionKind::Struct {
                generic_parameters, ..
            } => {
                let resolved_args: Vec<RT> = args
                    .iter()
                    .map(|a| resolve(db, tables, a, module, type_params, span, file_id))
                    .collect::<Option<Vec<_>>>()?;
                if resolved_args.len() != generic_parameters.len() {
                    TypeError::UnboundTypeParameter {
                        name: name.clone(),
                        span,
                        file_id,
                    }
                    .accumulate(db);
                    return None;
                }
                Some(RT::Parameterized(type_name, resolved_args))
            }
            _ => {
                TypeError::UnboundTypeParameter {
                    name: name.clone(),
                    span,
                    file_id,
                }
                .accumulate(db);
                None
            }
        },
        None => {
            TypeError::UnboundTypeParameter {
                name: name.clone(),
                span,
                file_id,
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
