use salsa::Accumulator;
use std::collections::HashMap;
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
}

#[salsa::tracked]
pub fn solve_constraints(db: &dyn TypeCheckDatabase, program: ProgramInput) -> SolvedConstraints {
    let constraints = check_program::accumulated::<Constraint>(db, program);
    let mut resolved: HashMap<String, HashMap<String, Vec<Type>>> = HashMap::new();

    for constraint in constraints {
        match &constraint.kind {
            ConstraintKind::TypeBound { .. } => {
                check_type_bound(db, &constraint, &mut resolved);
            }
            ConstraintKind::SigCheck { .. } => {
                check_sig_constraint(db, &constraint);
            }
        }
    }

    SolvedConstraints { resolved }
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
