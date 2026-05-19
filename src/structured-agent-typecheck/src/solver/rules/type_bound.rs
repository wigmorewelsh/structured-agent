use crate::TypeError;
use crate::db::TypeCheckDatabase;
use crate::solver::{Constraint, ConstraintKind, SolveResult, SolveRule, SolverState};

pub struct TypeBoundRule;

impl SolveRule for TypeBoundRule {
    fn applies(&self, constraint: &Constraint) -> bool {
        matches!(constraint.kind, ConstraintKind::TypeBound { .. })
    }

    fn apply(
        &self,
        constraint: &Constraint,
        _db: &dyn TypeCheckDatabase,
        state: &mut SolverState,
    ) -> SolveResult {
        let ConstraintKind::TypeBound {
            caller,
            callee,
            actual_type,
            bound_type,
            ..
        } = &constraint.kind
        else {
            unreachable!()
        };
        if !actual_type.is_assignable_to(bound_type) {
            SolveResult::Conflict(TypeError::TypeMismatch {
                expected: bound_type.to_string(),
                found: actual_type.to_string(),
                span: constraint.span,
                file_id: constraint.file_id,
            })
        } else {
            state
                .resolved
                .entry(caller.clone())
                .or_default()
                .entry(callee.clone())
                .or_default()
                .push(actual_type.clone());
            SolveResult::Solved
        }
    }
}

#[cfg(test)]
mod type_bound_rule_tests {
    use super::*;
    use crate::db::TypeCheckDb;
    use crate::solver::{Flavour, Solver};
    use structured_agent_ast::types::Span;
    use structured_agent_runtime::Type;

    fn type_bound(caller: &str, callee: &str, actual: Type, bound: Type) -> Constraint {
        Constraint {
            flavour: Flavour::Wanted,
            kind: ConstraintKind::TypeBound {
                caller: caller.to_string(),
                callee: callee.to_string(),
                actual_type: actual,
                bound_type: bound,
                context: String::new(),
            },
            span: Span { start: 0, end: 0 },
            file_id: 0,
        }
    }

    #[test]
    fn valid_bound_no_error_resolved_updated() {
        let db = TypeCheckDb::default();
        let mut solver = Solver::new();
        solver
            .worklist
            .push_back(type_bound("caller", "callee", Type::int(), Type::int()));
        solver.solve(&db);
        assert!(solver.errors.is_empty());
        assert_eq!(solver.state.resolved["caller"]["callee"], vec![Type::int()]);
    }

    #[test]
    fn invalid_bound_emits_error() {
        let db = TypeCheckDb::default();
        let mut solver = Solver::new();
        solver
            .worklist
            .push_back(type_bound("caller", "callee", Type::int(), Type::boolean()));
        solver.solve(&db);
        assert_eq!(solver.errors.len(), 1);
        assert!(!solver.state.resolved.contains_key("caller"));
    }

    #[test]
    fn multiple_valid_bounds_accumulated() {
        let db = TypeCheckDb::default();
        let mut solver = Solver::new();
        solver
            .worklist
            .push_back(type_bound("caller", "callee", Type::int(), Type::int()));
        solver
            .worklist
            .push_back(type_bound("caller", "callee", Type::int(), Type::int()));
        solver.solve(&db);
        assert!(solver.errors.is_empty());
        assert_eq!(solver.state.resolved["caller"]["callee"].len(), 2);
    }
}
