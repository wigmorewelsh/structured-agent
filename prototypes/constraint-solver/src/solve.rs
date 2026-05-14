use std::collections::HashMap;

use crate::generate::Constraint;
use crate::types::{Aliases, Type};

#[derive(Debug, Default)]
pub struct SolvedConstraints {
    pub call_subst: HashMap<usize, HashMap<String, Type>>,
    pub errors: Vec<String>,
}

pub fn solve(constraints: Vec<Constraint>, aliases: &Aliases) -> SolvedConstraints {
    let mut solved = SolvedConstraints::default();

    for constraint in constraints {
        match constraint {
            Constraint::Unify { call_site, var, ty } => {
                let subst = solved.call_subst.entry(call_site).or_default();
                match subst.get(&var) {
                    None => {
                        subst.insert(var, ty);
                    }
                    Some(existing) if *existing == ty => {}
                    Some(existing) => {
                        solved.errors.push(format!(
                            "call site {call_site}: {var} cannot unify {existing:?} with {ty:?}"
                        ));
                    }
                }
            }
            Constraint::Subtype {
                call_site,
                from,
                to,
            } => {
                if !aliases.is_subtype(&from, &to) {
                    solved.errors.push(format!(
                        "call site {call_site}: {from:?} is not assignable to {to:?}"
                    ));
                }
            }
        }
    }

    solved
}
