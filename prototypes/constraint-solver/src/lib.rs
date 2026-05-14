mod ast;
mod elaborate;
mod generate;
mod solve;
mod types;

pub use ast::{Expr, TypedExpr};
pub use elaborate::elaborate;
pub use generate::{generate, GenResult};
pub use solve::{solve, SolvedConstraints};
pub use types::{Aliases, FnSig, Type};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env() -> (HashMap<String, FnSig>, Aliases) {
        let mut fns = HashMap::new();
        fns.insert(
            "identity".into(),
            FnSig {
                type_params: vec!["T".into()],
                param_types: vec![Type::Generic("T".into())],
                return_type: Type::Generic("T".into()),
            },
        );
        fns.insert(
            "describe".into(),
            FnSig {
                type_params: vec![],
                param_types: vec![Type::Alias("Number".into())],
                return_type: Type::Str,
            },
        );
        fns.insert(
            "pair".into(),
            FnSig {
                type_params: vec!["T".into()],
                param_types: vec![Type::Generic("T".into()), Type::Generic("T".into())],
                return_type: Type::Generic("T".into()),
            },
        );

        let mut alias_map = HashMap::new();
        alias_map.insert("Number".into(), vec![Type::Float, Type::Double]);
        let aliases = Aliases(alias_map);

        (fns, aliases)
    }

    fn pipeline(expr: Expr) -> (Option<TypedExpr>, Vec<String>) {
        let (fns, aliases) = env();
        let (_, gen_result) = generate(&expr, &fns, &aliases);
        if !gen_result.errors.is_empty() {
            return (None, gen_result.errors);
        }
        let solved = solve(gen_result.constraints, &aliases);
        if !solved.errors.is_empty() {
            return (None, solved.errors);
        }
        let typed = elaborate(&expr, &fns, &aliases, &solved);
        (typed, vec![])
    }

    #[test]
    fn identity_float_resolves_return_to_float() {
        let expr = Expr::Call {
            name: "identity".into(),
            args: vec![Expr::FloatLit(1.0)],
            call_site: 0,
        };
        let (typed, errors) = pipeline(expr);
        assert!(errors.is_empty(), "{:?}", errors);
        let typed = typed.unwrap();
        assert_eq!(typed.ty(), &Type::Float);
    }

    #[test]
    fn identity_double_resolves_return_to_double() {
        let expr = Expr::Call {
            name: "identity".into(),
            args: vec![Expr::DoubleLit(1.0)],
            call_site: 1,
        };
        let (typed, errors) = pipeline(expr);
        assert!(errors.is_empty(), "{:?}", errors);
        assert_eq!(typed.unwrap().ty(), &Type::Double);
    }

    #[test]
    fn describe_float_passes_subtype_check() {
        let expr = Expr::Call {
            name: "describe".into(),
            args: vec![Expr::FloatLit(1.0)],
            call_site: 2,
        };
        let (typed, errors) = pipeline(expr);
        assert!(errors.is_empty(), "{:?}", errors);
        assert_eq!(typed.unwrap().ty(), &Type::Str);
    }

    #[test]
    fn describe_double_passes_subtype_check() {
        let expr = Expr::Call {
            name: "describe".into(),
            args: vec![Expr::DoubleLit(2.0)],
            call_site: 3,
        };
        let (typed, errors) = pipeline(expr);
        assert!(errors.is_empty(), "{:?}", errors);
        assert_eq!(typed.unwrap().ty(), &Type::Str);
    }

    #[test]
    fn describe_str_fails_subtype_check() {
        let expr = Expr::Call {
            name: "describe".into(),
            args: vec![Expr::StrLit("hello".into())],
            call_site: 4,
        };
        let (_, errors) = pipeline(expr);
        assert!(!errors.is_empty());
    }

    #[test]
    fn pair_same_type_resolves() {
        let expr = Expr::Call {
            name: "pair".into(),
            args: vec![Expr::FloatLit(1.0), Expr::FloatLit(2.0)],
            call_site: 5,
        };
        let (typed, errors) = pipeline(expr);
        assert!(errors.is_empty(), "{:?}", errors);
        assert_eq!(typed.unwrap().ty(), &Type::Float);
    }

    #[test]
    fn pair_mismatched_types_fails() {
        let expr = Expr::Call {
            name: "pair".into(),
            args: vec![Expr::FloatLit(1.0), Expr::DoubleLit(2.0)],
            call_site: 6,
        };
        let (_, errors) = pipeline(expr);
        assert!(!errors.is_empty());
    }

    #[test]
    fn generate_emits_unify_constraint_for_generics() {
        let (fns, aliases) = env();
        let expr = Expr::Call {
            name: "identity".into(),
            args: vec![Expr::FloatLit(1.0)],
            call_site: 0,
        };
        let (_, result) = generate(&expr, &fns, &aliases);
        assert!(result.errors.is_empty());
        assert_eq!(result.constraints.len(), 1);
        let c = &result.constraints[0];
        assert!(
            matches!(c, generate::Constraint::Unify { call_site: 0, var, ty: Type::Float } if var == "T")
        );
    }

    #[test]
    fn generate_emits_subtype_constraint_for_aliases() {
        let (fns, aliases) = env();
        let expr = Expr::Call {
            name: "describe".into(),
            args: vec![Expr::FloatLit(1.0)],
            call_site: 0,
        };
        let (_, result) = generate(&expr, &fns, &aliases);
        assert!(result.errors.is_empty());
        assert_eq!(result.constraints.len(), 1);
        assert!(matches!(
            &result.constraints[0],
            generate::Constraint::Subtype {
                from: Type::Float,
                ..
            }
        ));
    }

    #[test]
    fn solver_conflicts_on_mismatched_unify() {
        use generate::Constraint;
        let (_, aliases) = env();
        let constraints = vec![
            Constraint::Unify {
                call_site: 0,
                var: "T".into(),
                ty: Type::Float,
            },
            Constraint::Unify {
                call_site: 0,
                var: "T".into(),
                ty: Type::Double,
            },
        ];
        let solved = solve(constraints, &aliases);
        assert!(!solved.errors.is_empty());
    }

    #[test]
    fn solver_expands_alias_for_subtype() {
        use generate::Constraint;
        let (_, aliases) = env();
        let constraints = vec![Constraint::Subtype {
            call_site: 0,
            from: Type::Float,
            to: Type::Alias("Number".into()),
        }];
        let solved = solve(constraints, &aliases);
        assert!(solved.errors.is_empty(), "{:?}", solved.errors);
    }

    #[test]
    fn elaboration_does_not_run_unifier() {
        let (fns, aliases) = env();
        let expr = Expr::Call {
            name: "identity".into(),
            args: vec![Expr::FloatLit(1.0)],
            call_site: 0,
        };
        let (_, gen_result) = generate(&expr, &fns, &aliases);
        let solved = solve(gen_result.constraints, &aliases);
        let typed = elaborate(&expr, &fns, &aliases, &solved).unwrap();
        assert_eq!(typed.ty(), &Type::Float);
    }
}
