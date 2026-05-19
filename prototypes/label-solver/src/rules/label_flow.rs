use crate::constraint::{Constraint, ConstraintKind};
use crate::solver::{SolveResult, SolveRule, SolverState};
use crate::types::LabelExpr;

pub struct LabelFlowRule;

impl SolveRule for LabelFlowRule {
    fn applies(&self, constraint: &Constraint) -> bool {
        matches!(constraint.kind, ConstraintKind::LabelFlow { .. })
    }

    fn apply(&self, constraint: &Constraint, state: &mut SolverState) -> SolveResult {
        match &constraint.kind {
            ConstraintKind::LabelFlow {
                from: LabelExpr::Concrete(f),
                to: LabelExpr::Concrete(t),
            } => {
                if state.lattice.satisfies(f, t) {
                    SolveResult::Solved
                } else {
                    // TODO: Derived conflicts should be informational rather than hard errors.
                    // GHC suppresses error messages for Derived conflicts and only reports the
                    // root Wanted that caused them, avoiding cascading duplicate errors.
                    SolveResult::Conflict(format!(
                        "{:?} cannot flow to {:?}: trust level insufficient",
                        f, t
                    ))
                }
            }
            ConstraintKind::LabelFlow { .. } => SolveResult::Deferred,
            _ => unreachable!(),
        }
    }
}
