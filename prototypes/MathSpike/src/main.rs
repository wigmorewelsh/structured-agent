mod abt;
mod ast;
mod checker;
mod db;

use abt::{elaborate_fn, symbol_table};
use ast::{Expr, Op, Param, Type};
use checker::{FnDecl, Program, TypeError, check_program};
use db::Database;

fn ty(name: &str) -> Type {
    Type::Symbol(name.into())
}

fn lit(n: i64) -> Expr {
    Expr::Lit(n)
}

fn var(name: &str) -> Expr {
    Expr::Var(name.into())
}

fn add(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Op::Add, Box::new(lhs), Box::new(rhs))
}

fn mul(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Op::Mul, Box::new(lhs), Box::new(rhs))
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call(name.into(), args)
}

fn param(name: &str) -> Param {
    Param {
        name: name.into(),
        ty: None,
    }
}

fn param_typed(name: &str, ty_name: &str) -> Param {
    Param {
        name: name.into(),
        ty: Some(ty(ty_name)),
    }
}

fn main() {
    let db = Database::default();

    let double = FnDecl::new(
        &db,
        "double".into(),
        vec![param("x")],
        None,
        add(var("x"), var("x")),
    );

    let add_one = FnDecl::new(
        &db,
        "add_one".into(),
        vec![param_typed("x", "Int")],
        Some(ty("Int")),
        add(var("x"), lit(1)),
    );

    let quad = FnDecl::new(
        &db,
        "quad".into(),
        vec![param("x")],
        None,
        call("double", vec![call("double", vec![var("x")])]),
    );

    let fact = FnDecl::new(
        &db,
        "fact".into(),
        vec![param_typed("n", "Int")],
        Some(ty("Int")),
        mul(var("n"), call("fact", vec![var("n")])),
    );

    let bad = FnDecl::new(
        &db,
        "bad".into(),
        vec![param_typed("x", "Float")],
        Some(ty("Float")),
        var("x"),
    );

    let program = Program::new(&db, vec![double, add_one, quad, fact, bad]);

    check_program(&db, program);
    let errors = check_program::accumulated::<TypeError>(&db, program);

    if !errors.is_empty() {
        for e in &errors {
            println!("error: {}", e.0);
        }
    }

    let table = symbol_table(&db, program);
    let mut names: Vec<&String> = table.functions.keys().collect();
    names.sort();
    println!("Symbol table:");
    for name in names {
        let sig = &table.functions[name];
        let params: Vec<String> = sig.params.iter().map(|t| format!("{t:?}")).collect();
        println!("  {name}({}) -> {:?}", params.join(", "), sig.return_type);
    }

    println!("\nElaborated:");
    for &func in program.functions(&db) {
        if let Some(typed) = elaborate_fn(&db, program, func) {
            println!("  {} = {}", func.name(&db), typed);
        }
    }
}
