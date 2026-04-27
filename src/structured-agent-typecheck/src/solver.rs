use nonempty::NonEmpty;
use salsa::Accumulator;
use std::collections::HashMap;
use structured_agent_ast::CheckerAstRef;
use structured_agent_runtime::Type;
use structured_agent_runtime::symbols::{DefinitionPath, TypeDefinitionKind};

use crate::db::{ProgramInput, TypeCheckDatabase, check_program};
use crate::{TypeError, TypeErrorAccumulator};
use structured_agent_ast::types::Span;

#[derive(Clone, Debug, PartialEq)]
pub enum ConstraintKind {
    TypeBound {
        caller: String,
        callee: String,
        actual_type: Type,
        bound_type: Type,
        context: String,
    },
    SigCheck {
        module_type: DefinitionPath,
        sig_type: DefinitionPath,
    },
    TraitImpl {
        type_path: DefinitionPath,
        trait_path: DefinitionPath,
        impl_key: DefinitionPath,
    },
    TraitBound {
        type_path: DefinitionPath,
        trait_path: DefinitionPath,
        type_name: String,
        trait_name: String,
        param_name: String,
    },
}

#[salsa::accumulator]
#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    pub kind: ConstraintKind,
    pub span: Span,
    pub file_id: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SolvedConstraints {
    pub resolved: HashMap<String, HashMap<String, Vec<Type>>>,
    pub impls: HashMap<(DefinitionPath, DefinitionPath), DefinitionPath>,
}

#[salsa::tracked]
pub fn solve_constraints(db: &dyn TypeCheckDatabase, program: ProgramInput) -> SolvedConstraints {
    let constraints = check_program::accumulated::<Constraint>(db, program);
    let mut resolved: HashMap<String, HashMap<String, Vec<Type>>> = HashMap::new();
    let mut impls: HashMap<(DefinitionPath, DefinitionPath), DefinitionPath> = HashMap::new();

    for constraint in &constraints {
        match &constraint.kind {
            ConstraintKind::TypeBound { .. } => {
                check_type_bound(db, constraint, &mut resolved);
            }
            ConstraintKind::SigCheck { .. } => {
                check_sig_constraint(db, constraint);
            }
            ConstraintKind::TraitImpl { .. } => {
                check_trait_impl(db, constraint, &mut impls);
            }
            ConstraintKind::TraitBound { .. } => {}
        }
    }

    for (impl_key, impl_def) in db.symbol_tables().impls(db).get().iter() {
        if !matches!(impl_def.ast_ref, CheckerAstRef::Primitive) {
            continue;
        }
        let Some(trait_type) = &impl_def.trait_name else {
            continue;
        };
        let type_name = impl_def.type_name.name().to_string();
        let trait_name = trait_type.name().to_string();
        let local_type = DefinitionPath::for_type(impl_def.module.clone(), &type_name);
        let type_path = if db.symbol_tables().types(db).get().contains_key(&local_type) {
            local_type
        } else {
            DefinitionPath::for_type(
                DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
                &type_name,
            )
        };
        let trait_path = DefinitionPath::for_type(impl_def.module.clone(), &trait_name);
        impls.insert((type_path, trait_path), impl_key.clone());
    }

    for constraint in &constraints {
        if let ConstraintKind::TraitBound {
            type_path,
            trait_path,
            type_name,
            trait_name,
            param_name,
        } = &constraint.kind
        {
            if !impls.contains_key(&(type_path.clone(), trait_path.clone())) {
                TypeErrorAccumulator(TypeError::TraitBoundNotSatisfied {
                    type_name: type_name.clone(),
                    trait_name: trait_name.clone(),
                    param_name: param_name.clone(),
                    span: constraint.span,
                    file_id: constraint.file_id,
                })
                .accumulate(db);
            }
        }
    }

    SolvedConstraints { resolved, impls }
}

fn check_trait_impl(
    db: &dyn TypeCheckDatabase,
    constraint: &Constraint,
    impls: &mut HashMap<(DefinitionPath, DefinitionPath), DefinitionPath>,
) {
    let ConstraintKind::TraitImpl {
        type_path,
        trait_path,
        impl_key,
    } = &constraint.kind
    else {
        return;
    };

    let types = db.symbol_tables().types(db);
    let type_map = types.get();

    let trait_td = match type_map.get(trait_path) {
        Some(td) if matches!(&td.kind, TypeDefinitionKind::Trait { .. }) => td,
        _ => {
            TypeErrorAccumulator(TypeError::UnknownTrait {
                name: trait_path.last_name().to_string(),
                span: constraint.span,
                file_id: constraint.file_id,
            })
            .accumulate(db);
            return;
        }
    };

    let TypeDefinitionKind::Trait { functions, .. } = &trait_td.kind else {
        return;
    };

    let impls_table = db.symbol_tables().impls(db);
    let Some(impl_def) = impls_table.get().get(impl_key) else {
        return;
    };

    let fn_table = db.symbol_tables().functions(db);
    for sig_entry in functions {
        let has_fn = fn_table.get().keys().any(|fn_path| {
            fn_path.impl_key() == Some(impl_def.key.clone())
                && fn_path.last_name() == sig_entry.name
        });
        if !has_fn {
            TypeErrorAccumulator(TypeError::TraitImplMissingFunction {
                type_name: type_path.last_name().to_string(),
                trait_name: trait_path.last_name().to_string(),
                function_name: sig_entry.name.clone(),
                span: constraint.span,
                file_id: constraint.file_id,
            })
            .accumulate(db);
        }
    }

    impls.insert(
        (type_path.clone(), trait_path.clone()),
        impl_def.key.clone(),
    );
}

fn check_type_bound(
    db: &dyn TypeCheckDatabase,
    constraint: &Constraint,
    resolved: &mut HashMap<String, HashMap<String, Vec<Type>>>,
) {
    let ConstraintKind::TypeBound {
        caller,
        callee,
        actual_type,
        bound_type,
        ..
    } = &constraint.kind
    else {
        return;
    };
    if actual_type != bound_type {
        TypeErrorAccumulator(TypeError::TypeMismatch {
            expected: bound_type.to_string(),
            found: actual_type.to_string(),
            span: constraint.span,
            file_id: constraint.file_id,
        })
        .accumulate(db);
    } else {
        resolved
            .entry(caller.clone())
            .or_default()
            .entry(callee.clone())
            .or_default()
            .push(actual_type.clone());
    }
}

fn check_sig_constraint(db: &dyn TypeCheckDatabase, constraint: &Constraint) {
    let ConstraintKind::SigCheck {
        module_type,
        sig_type,
    } = &constraint.kind
    else {
        return;
    };
    let types_arc = db.symbol_tables().types(db);
    let type_map = types_arc.get();
    match (type_map.get(module_type), type_map.get(sig_type)) {
        (Some(module_td), Some(sig_td)) => {
            check_sig_entries(
                db,
                constraint,
                &module_td.kind,
                &sig_td.kind,
                sig_type,
                module_type,
            );
        }
        _ => {
            TypeErrorAccumulator(TypeError::TypeMismatch {
                expected: sig_type.to_string(),
                found: module_type.to_string(),
                span: constraint.span,
                file_id: constraint.file_id,
            })
            .accumulate(db);
        }
    }
}

fn check_sig_entries(
    db: &dyn TypeCheckDatabase,
    constraint: &Constraint,
    module_kind: &TypeDefinitionKind<crate::refs::CheckerRefs>,
    sig_kind: &TypeDefinitionKind<crate::refs::CheckerRefs>,
    sig_type: &DefinitionPath,
    module_type: &DefinitionPath,
) {
    match (module_kind, sig_kind) {
        (
            TypeDefinitionKind::Signature {
                entries: mod_entries,
            },
            TypeDefinitionKind::Signature {
                entries: sig_entries,
            },
        ) => {
            for sig_entry in sig_entries {
                if !mod_entries.iter().any(|e| e.name == sig_entry.name) {
                    TypeErrorAccumulator(TypeError::TypeMismatch {
                        expected: sig_entry.name.clone(),
                        found: String::from("missing"),
                        span: constraint.span,
                        file_id: constraint.file_id,
                    })
                    .accumulate(db);
                }
            }
        }
        _ => {
            TypeErrorAccumulator(TypeError::TypeMismatch {
                expected: sig_type.to_string(),
                found: module_type.to_string(),
                span: constraint.span,
                file_id: constraint.file_id,
            })
            .accumulate(db);
        }
    }
}
