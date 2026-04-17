use salsa::Accumulator;
use std::collections::HashMap;
use structured_agent_runtime::Type;

use crate::typecheck::db::{ProgramInput, SymbolTablesInput, TypeCheckDatabase, check_program};
use crate::typecheck::{TypeError, TypeErrorAccumulator};
use crate::types::Span;

#[salsa::accumulator]
#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    pub caller: String,
    pub callee: String,
    pub actual_type: Type,
    pub bound_type: Type,
    pub context: String,
    pub span: Span,
    pub file_id: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SolvedConstraints {
    pub resolved: HashMap<String, HashMap<String, Vec<Type>>>,
}

#[salsa::tracked]
pub fn solve_constraints(
    db: &dyn TypeCheckDatabase,
    program: ProgramInput,
    tables: SymbolTablesInput,
) -> SolvedConstraints {
    let constraints = check_program::accumulated::<Constraint>(db, program, tables);
    let mut resolved: HashMap<String, HashMap<String, Vec<Type>>> = HashMap::new();

    for constraint in constraints {
        if constraint.actual_type != constraint.bound_type {
            TypeErrorAccumulator(TypeError::TypeMismatch {
                expected: constraint.bound_type.to_string(),
                found: constraint.actual_type.to_string(),
                span: constraint.span,
                file_id: constraint.file_id,
            })
            .accumulate(db);
        } else {
            resolved
                .entry(constraint.caller.clone())
                .or_default()
                .entry(constraint.callee.clone())
                .or_default()
                .push(constraint.actual_type.clone());
        }
    }

    SolvedConstraints { resolved }
}
