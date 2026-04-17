#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Symbol(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<Type>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Expr {
    Lit(i64),
    Var(String),
    BinOp(Op, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Str(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Param {
    pub name: String,
    pub ty: Option<Type>,
}
