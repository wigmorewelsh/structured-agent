use std::collections::HashMap;

use crate::ast::{Expr, TypedExpr};
use crate::solve::SolvedConstraints;
use crate::types::{Aliases, FnSig};

pub fn elaborate(
    expr: &Expr,
    fns: &HashMap<String, FnSig>,
    aliases: &Aliases,
    solved: &SolvedConstraints,
) -> Option<TypedExpr> {
    match expr {
        Expr::FloatLit(v) => Some(TypedExpr::FloatLit(*v)),
        Expr::DoubleLit(v) => Some(TypedExpr::DoubleLit(*v)),
        Expr::StrLit(v) => Some(TypedExpr::StrLit(v.clone())),
        Expr::Call {
            name,
            args,
            call_site,
        } => {
            let sig = fns.get(name.as_str())?;
            let empty = HashMap::new();
            let subst = solved.call_subst.get(call_site).unwrap_or(&empty);
            let return_ty = aliases.apply_subst(&sig.return_type, subst);
            let typed_args = args
                .iter()
                .map(|a| elaborate(a, fns, aliases, solved))
                .collect::<Option<Vec<_>>>()?;
            Some(TypedExpr::Call {
                name: name.clone(),
                args: typed_args,
                return_ty,
            })
        }
    }
}
