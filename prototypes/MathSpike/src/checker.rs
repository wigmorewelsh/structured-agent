use std::collections::HashMap;

use salsa::Accumulator;

use crate::ast::{Expr, Param, Type};

#[salsa::accumulator]
#[derive(Clone, Debug, PartialEq)]
pub struct TypeError(pub String);

#[salsa::input]
pub struct FnDecl {
    #[returns(ref)]
    pub name: String,
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
        Type::Symbol(s) => error(db, format!("unknown type '{s}'")),
    }
}

fn param_type(db: &dyn salsa::Database, p: &Param) -> Type {
    p.ty.as_ref()
        .and_then(|t| resolve_type(db, t))
        .unwrap_or_else(int)
}

pub(crate) fn lookup_fn(db: &dyn salsa::Database, program: Program, name: &str) -> Option<FnDecl> {
    program
        .functions(db)
        .iter()
        .find(|f| f.name(db) == name)
        .copied()
}

pub(crate) fn build_env(params: &[Param], types: &[Type]) -> HashMap<String, Type> {
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
            synthesize(db, program, &env, func.body(db)).unwrap_or_else(int)
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
            TypeError(format!(
                "recursive function '{}' requires an explicit return type",
                func.name(db)
            ))
            .accumulate(db);
            return;
        }
    };

    if func.return_ty(db).is_some() {
        let env = build_env(func.params(db), &sig.params);
        check_expr(db, program, &env, func.body(db), &sig.return_type);
    }
}

#[salsa::tracked]
pub fn check_program(db: &dyn salsa::Database, program: Program) {
    for &func in program.functions(db) {
        check_fn(db, program, func);
    }
}

pub(crate) fn synthesize(
    db: &dyn salsa::Database,
    program: Program,
    env: &HashMap<String, Type>,
    expr: &Expr,
) -> Option<Type> {
    match expr {
        Expr::Lit(_) => Some(int()),
        Expr::Var(name) => env
            .get(name)
            .cloned()
            .or_else(|| error(db, format!("undefined variable '{name}'"))),
        Expr::BinOp(_, lhs, rhs) => {
            let lhs_ty = synthesize(db, program, env, lhs);
            let rhs_ty = synthesize(db, program, env, rhs);
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
            for (arg, expected) in args.iter().zip(&sig.params) {
                check_expr(db, program, env, arg, expected);
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
) -> Option<()> {
    match expr {
        Expr::BinOp(_, lhs, rhs) => {
            check_expr(db, program, env, lhs, expected)?;
            check_expr(db, program, env, rhs, expected)?;
            Some(())
        }
        _ => {
            let got = synthesize(db, program, env, expr)?;
            if got != *expected {
                error(db, format!("expected {expected:?}, got {got:?}"))
            } else {
                Some(())
            }
        }
    }
}
