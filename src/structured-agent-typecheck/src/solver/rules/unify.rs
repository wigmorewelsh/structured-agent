use crate::TypeError;
use crate::db::TypeCheckDatabase;
use crate::solver::{Constraint, ConstraintKind, SolveResult, SolveRule, SolverState};

pub struct UnifyRule;

impl SolveRule for UnifyRule {
    fn applies(&self, constraint: &Constraint) -> bool {
        matches!(constraint.kind, ConstraintKind::Unify { .. })
    }

    fn apply(
        &self,
        constraint: &Constraint,
        _db: &dyn TypeCheckDatabase,
        state: &mut SolverState,
    ) -> SolveResult {
        let ConstraintKind::Unify {
            call_site,
            var,
            ty,
            fn_name,
            param_name,
        } = &constraint.kind
        else {
            unreachable!()
        };
        let slot = state
            .generic_solutions
            .entry((constraint.file_id, *call_site))
            .or_default();
        match slot.get(var) {
            Some(existing) if existing != ty => {
                SolveResult::Conflict(TypeError::ArgumentTypeMismatch {
                    function: fn_name.clone(),
                    parameter: param_name.clone(),
                    expected: existing.to_string(),
                    found: ty.to_string(),
                    span: constraint.span,
                    file_id: constraint.file_id,
                })
            }
            _ => {
                slot.insert(var.clone(), ty.clone());
                SolveResult::Solved
            }
        }
    }
}

#[cfg(test)]
mod unify_rule_tests {
    use super::*;
    use crate::db::TypeCheckDb;
    use crate::solver::{Flavour, Solver};
    use structured_agent_ast::types::Span;
    use structured_agent_runtime::Type;

    fn unify(file_id: usize, call_site: usize, var: &str, ty: Type) -> Constraint {
        Constraint {
            flavour: Flavour::Wanted,
            kind: ConstraintKind::Unify {
                call_site,
                var: var.to_string(),
                ty,
                fn_name: "f".to_string(),
                param_name: "x".to_string(),
            },
            span: Span { start: 0, end: 0 },
            file_id,
        }
    }

    #[test]
    fn consistent_unify_is_solved() {
        let db = TypeCheckDb::default();
        let mut solver = Solver::new();
        solver.worklist.push_back(unify(0, 0, "T", Type::int()));
        solver.solve(&db);
        assert!(solver.errors.is_empty());
        assert_eq!(solver.state.generic_solutions[&(0, 0)]["T"], Type::int());
    }

    #[test]
    fn identical_unify_is_idempotent() {
        let db = TypeCheckDb::default();
        let mut solver = Solver::new();
        solver.worklist.push_back(unify(0, 0, "T", Type::int()));
        solver.worklist.push_back(unify(0, 0, "T", Type::int()));
        solver.solve(&db);
        assert!(solver.errors.is_empty());
        assert_eq!(solver.state.generic_solutions[&(0, 0)].len(), 1);
    }

    #[test]
    fn conflicting_unify_emits_error() {
        let db = TypeCheckDb::default();
        let mut solver = Solver::new();
        solver.worklist.push_back(unify(0, 0, "T", Type::int()));
        solver.worklist.push_back(unify(0, 0, "T", Type::boolean()));
        solver.solve(&db);
        assert_eq!(solver.errors.len(), 1);
        assert_eq!(solver.state.generic_solutions[&(0, 0)]["T"], Type::int());
    }

    #[test]
    fn different_call_sites_are_independent() {
        let db = TypeCheckDb::default();
        let mut solver = Solver::new();
        solver.worklist.push_back(unify(0, 0, "T", Type::int()));
        solver.worklist.push_back(unify(0, 1, "T", Type::boolean()));
        solver.solve(&db);
        assert!(solver.errors.is_empty());
        assert_eq!(solver.state.generic_solutions[&(0, 0)]["T"], Type::int());
        assert_eq!(
            solver.state.generic_solutions[&(0, 1)]["T"],
            Type::boolean()
        );
    }

    #[test]
    fn different_vars_same_call_site() {
        let db = TypeCheckDb::default();
        let mut solver = Solver::new();
        solver.worklist.push_back(unify(0, 0, "T", Type::int()));
        solver.worklist.push_back(unify(0, 0, "U", Type::boolean()));
        solver.solve(&db);
        assert!(solver.errors.is_empty());
        assert_eq!(solver.state.generic_solutions[&(0, 0)]["T"], Type::int());
        assert_eq!(
            solver.state.generic_solutions[&(0, 0)]["U"],
            Type::boolean()
        );
    }
}
