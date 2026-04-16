use super::db::{
    Intern, InternedModuleName, InternedString, SymbolTablesInput, TypeCheckDatabase,
    resolve_type_alias,
};
use super::{CheckContext, TypeEnvironment};

use crate::ast::Type as AstType;
use crate::ensure_or_accumulate;
use crate::typecheck::error::{OrAccumulateError, TypeError};
use crate::types::Span;

use std::collections::HashMap;
use structured_agent_runtime::Type as RT;
use structured_agent_runtime::symbols::{TypeDefinitionKind, TypeName};

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
