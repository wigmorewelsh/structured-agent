use crate::constraint::{Constraint, ConstraintKind};
use crate::solver::{SolveResult, SolveRule, SolverState};

pub struct LabelUnifyRule;

impl SolveRule for LabelUnifyRule {
    fn applies(&self, constraint: &Constraint) -> bool {
        matches!(constraint.kind, ConstraintKind::LabelUnify { .. })
    }

    fn apply(&self, constraint: &Constraint, state: &mut SolverState) -> SolveResult {
        match &constraint.kind {
            ConstraintKind::LabelUnify { var, label } => {
                let existing = state.substitution.get(var).cloned();
                match existing {
                    Some(e) if e == *label => SolveResult::Solved,
                    Some(e) => SolveResult::Conflict(format!(
                        "label variable '{}' already bound to {:?}, cannot rebind to {:?}",
                        var, e, label
                    )),
                    None => {
                        state.substitution.insert(var.clone(), label.clone());
                        let derived = state.derive_from_inert(var, label);
                        // TODO: Stale inert entries are not removed after unification (no kick-out).
                        // GHC's kick-out mechanism removes inert constraints that become actionable
                        // when a new substitution is added. Without it, the original LabelFlow
                        // with the unresolved variable remains in the inert set. In a larger solver
                        // this causes the inert set to grow monotonically and risks false
                        // "unresolved Wanted" errors on constraints that were actually discharged
                        // via their Derived counterpart.
                        if derived.is_empty() {
                            SolveResult::Solved
                        } else {
                            SolveResult::Emits(derived)
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}
