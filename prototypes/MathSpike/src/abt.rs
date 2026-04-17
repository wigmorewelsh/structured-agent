use std::collections::HashMap;
use std::fmt;

use crate::ast::{Op, Type};
use crate::checker::{FnDecl, FnSig, Program, build_env, lookup_fn, query_sig};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VarId {
    pub fn_name: String,
    pub param_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypedExprKind {
    Lit(i64),
    Str(String),
    Var(VarId),
    BinOp(Op, Box<TypedExpr>, Box<TypedExpr>),
    Call(String, Vec<Type>, Vec<TypedExpr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolTable {
    pub functions: HashMap<String, FnSig>,
}

#[salsa::tracked]
pub fn symbol_table(db: &dyn salsa::Database, program: Program) -> SymbolTable {
    let functions = program
        .functions(db)
        .iter()
        .filter_map(|&f| query_sig(db, program, f).map(|sig| (f.name(db).clone(), sig)))
        .collect();
    SymbolTable { functions }
}

#[salsa::tracked]
pub fn elaborate_fn(db: &dyn salsa::Database, program: Program, func: FnDecl) -> Option<TypedExpr> {
    let sig = query_sig(db, program, func)?;
    let var_env: HashMap<String, VarId> = func
        .params(db)
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                p.name.clone(),
                VarId {
                    fn_name: func.name(db).clone(),
                    param_index: i,
                },
            )
        })
        .collect();
    let type_env = build_env(func.params(db), &sig.params);
    let solved = crate::checker::solve_constraints(db, program);
    let call_types = solved.resolved.get(func.name(db)).cloned().unwrap_or_default();
    elaborate_expr(db, program, &var_env, &type_env, func.body(db), &call_types)
}

fn elaborate_expr(
    db: &dyn salsa::Database,
    program: Program,
    var_env: &HashMap<String, VarId>,
    type_env: &HashMap<String, Type>,
    expr: &crate::ast::Expr,
    call_types: &HashMap<String, Vec<Type>>,
) -> Option<TypedExpr> {
    match expr {
        crate::ast::Expr::Lit(n) => Some(TypedExpr {
            kind: TypedExprKind::Lit(*n),
            ty: Type::Symbol("Int".into()),
        }),
        crate::ast::Expr::Str(s) => Some(TypedExpr {
            kind: TypedExprKind::Str(s.clone()),
            ty: Type::Symbol("String".into()),
        }),
        crate::ast::Expr::Var(name) => {
            let id = var_env.get(name)?;
            Some(TypedExpr {
                kind: TypedExprKind::Var(id.clone()),
                ty: Type::Symbol("Int".into()),
            })
        }
        crate::ast::Expr::BinOp(op, lhs, rhs) => {
            let lhs = elaborate_expr(db, program, var_env, type_env, lhs, call_types)?;
            let rhs = elaborate_expr(db, program, var_env, type_env, rhs, call_types)?;
            Some(TypedExpr {
                kind: TypedExprKind::BinOp(op.clone(), Box::new(lhs), Box::new(rhs)),
                ty: Type::Symbol("Int".into()),
            })
        }
        crate::ast::Expr::Call(name, args) => {
            let func = lookup_fn(db, program, name)?;
            let sig = query_sig(db, program, func)?;
            let elaborated: Option<Vec<TypedExpr>> = args
                .iter()
                .map(|a| elaborate_expr(db, program, var_env, type_env, a, call_types))
                .collect();
            let t_args = call_types.get(name).cloned().unwrap_or_default();
            Some(TypedExpr {
                kind: TypedExprKind::Call(name.clone(), t_args, elaborated?),
                ty: sig.return_type,
            })
        }
    }
}

impl fmt::Display for TypedExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TypedExprKind::Lit(n) => write!(f, "{n}"),
            TypedExprKind::Str(s) => write!(f, "{s:?}"),
            TypedExprKind::Var(v) => write!(f, "{}#{}", v.fn_name, v.param_index),
            TypedExprKind::BinOp(op, lhs, rhs) => {
                let sym = match op {
                    Op::Add => "+",
                    Op::Sub => "-",
                    Op::Mul => "*",
                    Op::Div => "/",
                };
                write!(f, "({lhs} {sym} {rhs})")
            }
            TypedExprKind::Call(name, t_args, args) => {
                let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                if t_args.is_empty() {
                    write!(f, "{name}({})", args.join(", "))
                } else {
                    let ts: Vec<String> = t_args.iter().map(|t| match t {
                        Type::Symbol(s) => s.clone(),
                    }).collect();
                    write!(f, "{name}<{}>({})", ts.join(", "), args.join(", "))
                }
            }
        }
    }
}
