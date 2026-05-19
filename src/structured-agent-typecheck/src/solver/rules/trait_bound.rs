use crate::db::TypeCheckDatabase;
use crate::solver::{Constraint, ConstraintKind, SolveResult, SolveRule, SolverState};

pub struct TraitBoundRule;

impl SolveRule for TraitBoundRule {
    fn applies(&self, constraint: &Constraint) -> bool {
        matches!(constraint.kind, ConstraintKind::TraitBound { .. })
    }

    fn apply(
        &self,
        constraint: &Constraint,
        _db: &dyn TypeCheckDatabase,
        state: &mut SolverState,
    ) -> SolveResult {
        let ConstraintKind::TraitBound {
            type_path,
            trait_path,
            ..
        } = &constraint.kind
        else {
            unreachable!()
        };
        if state
            .impls
            .contains_key(&(type_path.clone(), trait_path.clone()))
        {
            SolveResult::Solved
        } else {
            SolveResult::Deferred
        }
    }
}

#[cfg(test)]
mod trait_bound_rule_tests {
    use super::*;
    use crate::solver::{Flavour, Solver};
    use nonempty::NonEmpty;
    use structured_agent_runtime::symbols::DefinitionPath;

    #[test]
    fn satisfied_trait_bound_is_solved() {
        let db = crate::db::TypeCheckDb::default();
        let type_path = DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new("m".to_string())),
            "MyType",
        );
        let trait_path = DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new("m".to_string())),
            "MyTrait",
        );
        let mut solver = Solver::new();
        solver
            .state
            .impls
            .insert((type_path.clone(), trait_path.clone()), type_path.clone());
        solver.worklist.push_back(Constraint {
            flavour: Flavour::Wanted,
            kind: ConstraintKind::TraitBound {
                type_path: type_path.clone(),
                trait_path: trait_path.clone(),
                type_name: "MyType".to_string(),
                trait_name: "MyTrait".to_string(),
                param_name: "T".to_string(),
            },
            span: structured_agent_ast::types::Span { start: 0, end: 0 },
            file_id: 0,
        });
        solver.solve(&db);
        assert!(solver.errors.is_empty());
        assert!(solver.state.inert.is_empty());
    }
}
