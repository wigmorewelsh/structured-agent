mod abt;
mod ast;
mod checker;
mod db;

use abt::{elaborate_fn, symbol_table};
use ast::{Expr, Op, Param, Type, TypeParam};
use checker::{FnDecl, Program, TypeError, ConstraintError, solve_constraints, check_program};
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

fn type_param(name: &str, bound: &str) -> TypeParam {
    TypeParam {
        name: name.into(),
        bounds: vec![ty(bound)],
    }
}

fn main() {
    let db = Database::default();

    let double = FnDecl::new(
        &db,
        "double".into(),
        vec![],
        vec![param("x")],
        None,
        add(var("x"), var("x")),
    );

    let add_one = FnDecl::new(
        &db,
        "add_one".into(),
        vec![],
        vec![param_typed("x", "Int")],
        Some(ty("Int")),
        add(var("x"), lit(1)),
    );

    let quad = FnDecl::new(
        &db,
        "quad".into(),
        vec![],
        vec![param("x")],
        None,
        call("double", vec![call("double", vec![var("x")])]),
    );

    let fact = FnDecl::new(
        &db,
        "fact".into(),
        vec![],
        vec![param_typed("n", "Int")],
        Some(ty("Int")),
        mul(var("n"), call("fact", vec![var("n")])),
    );

    let bad = FnDecl::new(
        &db,
        "bad".into(),
        vec![],
        vec![param_typed("x", "Float")],
        Some(ty("Float")),
        var("x"),
    );

    let add_generic = FnDecl::new(
        &db,
        "add_generic".into(),
        vec![type_param("T", "Int")],
        vec![param("x"), param("y")],
        Some(ty("Int")),
        add(var("x"), var("y")),
    );

    let call_ok = FnDecl::new(
        &db,
        "call_ok".into(),
        vec![],
        vec![],
        Some(ty("Int")),
        call("add_generic", vec![lit(1), lit(2)]),
    );

    let call_bad = FnDecl::new(
        &db,
        "call_bad".into(),
        vec![],
        vec![],
        Some(ty("Int")),
        call("add_generic", vec![Expr::Str("hi".into()), Expr::Str("there".into())]),
    );

    let program = Program::new(
        &db,
        vec![double, add_one, quad, fact, bad, add_generic, call_ok, call_bad],
    );


    println!("--- Phase 1: Collection (Symbol Table) ---");
    let table = symbol_table(&db, program);
    let mut names: Vec<&String> = table.functions.keys().collect();
    names.sort();
    for name in names {
        let sig = &table.functions[name];
        let params: Vec<String> = sig.params.iter().map(|t| format!("{t:?}")).collect();
        println!("  {name}({}) -> {:?}", params.join(", "), sig.return_type);
    }

    println!("\n--- Phase 2: Synthesis and Checking ---");
    check_program(&db, program);
    let check_errors = check_program::accumulated::<TypeError>(&db, program);
    if !check_errors.is_empty() {
        for e in &check_errors {
            println!("  error: {}", e.0);
        }
    } else {
        println!("  (No synthesis errors)");
    }

    println!("\n--- Phase 3: Constraint Solving ---");
    solve_constraints(&db, program);
    let constraint_errors = solve_constraints::accumulated::<ConstraintError>(&db, program);
    if !constraint_errors.is_empty() {
        for e in &constraint_errors {
            println!("  constraint error: {}", e.0);
        }
    } else {
        println!("  (No constraint errors)");
    }

    println!("\n--- Phase 4: Elaboration ---");
    for &func in program.functions(&db) {
        if let Some(typed) = elaborate_fn(&db, program, func) {
            println!("  {} = {}", func.name(&db), typed);
        }
    }
}
