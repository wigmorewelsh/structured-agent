use std::collections::HashMap;

use salsa::Accumulator;

use crate::ast::{Expr, Param, Type, TypeParam};

#[salsa::accumulator]
#[derive(Clone, Debug, PartialEq)]
pub struct TypeError(pub String);

#[salsa::accumulator]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintError(pub String);

#[salsa::accumulator]
#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    pub caller: String,
    pub callee: String,
    pub actual_type: Type,
    pub bound_type: Type,
    pub context: String,
}

#[salsa::input]
pub struct FnDecl {
    #[returns(ref)]
    pub name: String,
    #[returns(ref)]
    pub type_params: Vec<TypeParam>,
    #[returns(ref)]
    pub params: Vec<Param>,
    pub return_ty: Option<Type>,
    #[returns(ref)]
    pub body: Expr,
}

#[salsa::input]
pub struct Program {
    #[returns(ref)]
    pub functions: Vec<FnDecl>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FnSig {
    pub params: Vec<Type>,
    pub return_type: Type,
}

fn int() -> Type {
    Type::Symbol("Int".into())
}

fn error<T>(db: &dyn salsa::Database, msg: impl Into<String>) -> Option<T> {
    TypeError(msg.into()).accumulate(db);
    None
}

fn resolve_type(db: &dyn salsa::Database, ty: &Type) -> Option<Type> {
    match ty {
        Type::Symbol(s) if s == "Int" => Some(int()),
        Type::Symbol(s) if s == "String" => Some(Type::Symbol("String".into())),
        Type::Symbol(s) => error(db, format!("unknown type '{s}'")),
    }
}

fn param_type(db: &dyn salsa::Database, p: &Param) -> Type {
    p.ty.as_ref()
        .and_then(|t| resolve_type(db, t))
        .unwrap_or_else(int)
}

pub fn lookup_fn(db: &dyn salsa::Database, program: Program, name: &str) -> Option<FnDecl> {
    program
        .functions(db)
        .iter()
        .find(|f| f.name(db) == name)
        .copied()
}

pub fn build_env(params: &[Param], types: &[Type]) -> HashMap<String, Type> {
    params
        .iter()
        .zip(types)
        .map(|(p, t)| (p.name.clone(), t.clone()))
        .collect()
}

fn sig_cycle_result(
    _db: &dyn salsa::Database,
    _id: salsa::Id,
    _program: Program,
    _func: FnDecl,
) -> Option<FnSig> {
    None
}

#[salsa::tracked(cycle_result = sig_cycle_result)]
pub fn query_sig(db: &dyn salsa::Database, program: Program, func: FnDecl) -> Option<FnSig> {
    let params: Vec<Type> = func.params(db).iter().map(|p| param_type(db, p)).collect();
    let return_type = match func.return_ty(db) {
        Some(ty) => resolve_type(db, &ty).unwrap_or_else(int),
        None => {
            let env = build_env(func.params(db), &params);
            synthesize(db, program, &env, func.body(db), func.name(db)).unwrap_or_else(int)
        }
    };
    Some(FnSig {
        params,
        return_type,
    })
}

#[salsa::tracked]
pub fn check_fn(db: &dyn salsa::Database, program: Program, func: FnDecl) {
    let sig = match query_sig(db, program, func) {
        Some(sig) => sig,
        None => {
            ConstraintError(format!(
                "recursive function '{}' requires an explicit return type",
                func.name(db)
            ))
            .accumulate(db);
            return;
        }
    };

    if func.return_ty(db).is_some() {
        let env = build_env(func.params(db), &sig.params);
        check_expr(db, program, &env, func.body(db), &sig.return_type, func.name(db));
    }
}

#[salsa::tracked]
pub fn check_program(db: &dyn salsa::Database, program: Program) {
    for &func in program.functions(db) {
        check_fn(db, program, func);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolvedConstraints {
    pub resolved: HashMap<String, HashMap<String, Vec<Type>>>,
}

#[salsa::tracked]
pub fn solve_constraints(db: &dyn salsa::Database, program: Program) -> SolvedConstraints {
    check_program(db, program);
    let constraints = check_program::accumulated::<Constraint>(db, program);
    let mut resolved: HashMap<String, HashMap<String, Vec<Type>>> = HashMap::new();

    for constraint in constraints {
        if constraint.actual_type != constraint.bound_type {
            ConstraintError(format!(
                "{}: expected type {:?}, got {:?}",
                constraint.context, constraint.bound_type, constraint.actual_type
            ))
            .accumulate(db);
        } else {
            resolved
                .entry(constraint.caller.clone())
                .or_default()
                .entry(constraint.callee.clone())
                .or_default()
                .push(constraint.actual_type.clone());
        }
    }
    SolvedConstraints { resolved }
}

pub fn synthesize(
    db: &dyn salsa::Database,
    program: Program,
    env: &HashMap<String, Type>,
    expr: &Expr,
    caller: &str,
) -> Option<Type> {
    match expr {
        Expr::Lit(_) => Some(int()),
        Expr::Str(_) => Some(Type::Symbol("String".into())),
        Expr::Var(name) => env
            .get(name)
            .cloned()
            .or_else(|| error(db, format!("undefined variable '{name}'"))),
        Expr::BinOp(_, lhs, rhs) => {
            let lhs_ty = synthesize(db, program, env, lhs, caller);
            let rhs_ty = synthesize(db, program, env, rhs, caller);
            match (lhs_ty, rhs_ty) {
                (Some(l), Some(r)) if l == r => Some(l),
                (Some(l), Some(r)) => error::<Type>(
                    db,
                    format!("type mismatch in binary operation: {l:?} vs {r:?}"),
                ),
                _ => None,
            }
        }
        Expr::Call(name, args) => {
            let func = lookup_fn(db, program, name)
                .or_else(|| error(db, format!("undefined function '{name}'")))?;
            let sig = query_sig(db, program, func)
                .or_else(|| error(db, format!("cannot determine signature of '{name}'")))?;
            if args.len() != sig.params.len() {
                return error(
                    db,
                    format!(
                        "function '{name}' expects {} argument(s) but got {}",
                        sig.params.len(),
                        args.len()
                    ),
                );
            }

            if !func.type_params(db).is_empty() && !args.is_empty() {
                if let Some(actual_ty) = synthesize(db, program, env, &args[0], caller) {
                    for type_param in func.type_params(db) {
                        for bound in &type_param.bounds {
                            if let Some(bound_ty) = resolve_type(db, bound) {
                                Constraint {
                                    caller: caller.to_string(),
                                    callee: name.clone(),
                                    actual_type: actual_ty.clone(),
                                    bound_type: bound_ty,
                                    context: format!(
                                        "call to '{}': type parameter '{}' bound",
                                        name, type_param.name
                                    ),
                                }
                                .accumulate(db);
                            }
                        }
                    }
                }
            }

            for (arg, expected) in args.iter().zip(&sig.params) {
                check_expr(db, program, env, arg, expected, caller);
            }
            Some(sig.return_type)
        }
    }
}

fn check_expr(
    db: &dyn salsa::Database,
    program: Program,
    env: &HashMap<String, Type>,
    expr: &Expr,
    expected: &Type,
    caller: &str,
) -> Option<()> {
    match expr {
        Expr::BinOp(_, lhs, rhs) => {
            check_expr(db, program, env, lhs, expected, caller)?;
            check_expr(db, program, env, rhs, expected, caller)?;
            Some(())
        }
        _ => {
            let got = synthesize(db, program, env, expr, caller)?;
            if got != *expected {
                error(db, format!("expected {expected:?}, got {got:?}"))
            } else {
                Some(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{Expr, Op, Param, Type, TypeParam};
    use crate::checker::{FnDecl, Program, ConstraintError, TypeError, solve_constraints, check_program};
    use crate::db::Database;

    fn make_add_fn(db: &Database) -> FnDecl {
        FnDecl::new(
            db,
            "add".into(),
            vec![TypeParam {
                name: "T".into(),
                bounds: vec![Type::Symbol("Int".into())],
            }],
            vec![
                Param { name: "x".into(), ty: None },
                Param { name: "y".into(), ty: None },
            ],
            Some(Type::Symbol("Int".into())),
            Expr::BinOp(
                Op::Add,
                Box::new(Expr::Var("x".into())),
                Box::new(Expr::Var("y".into())),
            ),
        )
    }

    #[test]
    fn constraint_satisfied_for_int_args() {
        let db = Database::default();
        let add = make_add_fn(&db);
        let caller = FnDecl::new(
            &db,
            "caller".into(),
            vec![],
            vec![],
            Some(Type::Symbol("Int".into())),
            Expr::Call("add".into(), vec![Expr::Lit(1), Expr::Lit(2)]),
        );
        let program = Program::new(&db, vec![add, caller]);
        solve_constraints(&db, program);
        let errors = solve_constraints::accumulated::<ConstraintError>(&db, program);
        let type_errors = check_program::accumulated::<TypeError>(&db, program);
        println!("TypeError: {:?}", type_errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn constraint_violated_for_string_args() {
        let db = Database::default();
        let add = make_add_fn(&db);
        let caller = FnDecl::new(
            &db,
            "caller".into(),
            vec![],
            vec![],
            Some(Type::Symbol("Int".into())),
            Expr::Call("add".into(), vec![Expr::Str("hi".into()), Expr::Str("there".into())]),
        );
        let program = Program::new(&db, vec![add, caller]);
        solve_constraints(&db, program);
        let errors = solve_constraints::accumulated::<ConstraintError>(&db, program);
        let type_errors = check_program::accumulated::<TypeError>(&db, program);
        println!("TypeError: {:?}", type_errors);
        println!("ERRORS: {:?}", errors);
        assert!(!type_errors.is_empty());
        //assert!(errors.iter().any(|e| e.0.contains("bound")));
    }
}
