use crate::TypeError;
use crate::db::TypeCheckDatabase;
use crate::solver::{Constraint, ConstraintKind, SolveResult, SolveRule, SolverState};
use structured_agent_runtime::symbols::TypeDefinitionKind;

pub struct TraitImplRule;

impl SolveRule for TraitImplRule {
    fn applies(&self, constraint: &Constraint) -> bool {
        matches!(constraint.kind, ConstraintKind::TraitImpl { .. })
    }

    fn apply(
        &self,
        constraint: &Constraint,
        db: &dyn TypeCheckDatabase,
        state: &mut SolverState,
    ) -> SolveResult {
        let ConstraintKind::TraitImpl {
            type_path,
            trait_path,
            impl_key,
        } = &constraint.kind
        else {
            unreachable!()
        };

        let types = db.symbol_tables().types(db);
        let type_map = types.get();

        let trait_td = match type_map.get(trait_path) {
            Some(td) if matches!(&td.kind, TypeDefinitionKind::Trait { .. }) => td,
            _ => {
                return SolveResult::Conflict(TypeError::UnknownTrait {
                    name: trait_path.last_name().to_string(),
                    span: constraint.span,
                    file_id: constraint.file_id,
                });
            }
        };

        let TypeDefinitionKind::Trait { functions, .. } = &trait_td.kind else {
            unreachable!()
        };

        let impls_table = db.symbol_tables().impls(db);
        let Some(impl_def) = impls_table.get().get(impl_key) else {
            return SolveResult::Solved;
        };

        let fn_table = db.symbol_tables().functions(db);
        for sig_entry in functions {
            let has_fn = fn_table.get().keys().any(|fn_path| {
                fn_path.impl_key() == Some(impl_def.key.clone())
                    && fn_path.last_name() == sig_entry.name
            });
            if !has_fn {
                return SolveResult::Conflict(TypeError::TraitImplMissingFunction {
                    type_name: type_path.last_name().to_string(),
                    trait_name: trait_path.last_name().to_string(),
                    function_name: sig_entry.name.clone(),
                    span: constraint.span,
                    file_id: constraint.file_id,
                });
            }
        }

        state.impls.insert(
            (type_path.clone(), trait_path.clone()),
            impl_def.key.clone(),
        );

        let mut kicked = Vec::new();
        state.inert.retain(|c| {
            if let ConstraintKind::TraitBound {
                type_path: tp,
                trait_path: trp,
                ..
            } = &c.kind
            {
                if tp == type_path && trp == trait_path {
                    kicked.push(c.clone());
                    return false;
                }
            }
            true
        });

        if kicked.is_empty() {
            SolveResult::Solved
        } else {
            SolveResult::Emits(kicked)
        }
    }
}
