use std::collections::HashMap;

use crate::ast::Expr;
use crate::types::{Aliases, FnSig, Type};

#[derive(Debug, Clone)]
pub enum Constraint {
    Unify {
        call_site: usize,
        var: String,
        ty: Type,
    },
    Subtype {
        call_site: usize,
        from: Type,
        to: Type,
    },
}

#[derive(Debug, Default)]
pub struct GenResult {
    pub constraints: Vec<Constraint>,
    pub errors: Vec<String>,
}

impl GenResult {
    fn merge(&mut self, other: GenResult) {
        self.constraints.extend(other.constraints);
        self.errors.extend(other.errors);
    }
}

pub fn generate(
    expr: &Expr,
    fns: &HashMap<String, FnSig>,
    aliases: &Aliases,
) -> (Option<Type>, GenResult) {
    match expr {
        Expr::FloatLit(_) => (Some(Type::Float), GenResult::default()),
        Expr::DoubleLit(_) => (Some(Type::Double), GenResult::default()),
        Expr::StrLit(_) => (Some(Type::Str), GenResult::default()),
        Expr::Call {
            name,
            args,
            call_site,
        } => {
            let Some(sig) = fns.get(name.as_str()) else {
                return (
                    None,
                    GenResult {
                        errors: vec![format!("unknown function: {name}")],
                        ..Default::default()
                    },
                );
            };
            let sig = sig.clone();
            let mut result = GenResult::default();

            if args.len() != sig.param_types.len() {
                result.errors.push(format!(
                    "arity mismatch for {name}: expected {}, got {}",
                    sig.param_types.len(),
                    args.len()
                ));
                return (None, result);
            }

            let mut arg_types = Vec::new();
            for arg in args {
                let (ty, child) = generate(arg, fns, aliases);
                result.merge(child);
                match ty {
                    Some(t) => arg_types.push(t),
                    None => return (None, result),
                }
            }

            for (param_ty, arg_ty) in sig.param_types.iter().zip(&arg_types) {
                match param_ty {
                    Type::Generic(var) => {
                        result.constraints.push(Constraint::Unify {
                            call_site: *call_site,
                            var: var.clone(),
                            ty: arg_ty.clone(),
                        });
                    }
                    _ => {
                        result.constraints.push(Constraint::Subtype {
                            call_site: *call_site,
                            from: arg_ty.clone(),
                            to: param_ty.clone(),
                        });
                    }
                }
            }

            (Some(sig.return_type.clone()), result)
        }
    }
}
