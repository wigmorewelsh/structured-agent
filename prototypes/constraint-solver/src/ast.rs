use crate::types::Type;

#[derive(Debug, Clone)]
pub enum Expr {
    FloatLit(f64),
    DoubleLit(f64),
    StrLit(String),
    Call {
        name: String,
        args: Vec<Expr>,
        call_site: usize,
    },
}

#[derive(Debug, Clone)]
pub enum TypedExpr {
    FloatLit(f64),
    DoubleLit(f64),
    StrLit(String),
    Call {
        name: String,
        args: Vec<TypedExpr>,
        return_ty: Type,
    },
}

impl TypedExpr {
    pub fn ty(&self) -> &Type {
        match self {
            TypedExpr::FloatLit(_) => &Type::Float,
            TypedExpr::DoubleLit(_) => &Type::Double,
            TypedExpr::StrLit(_) => &Type::Str,
            TypedExpr::Call { return_ty, .. } => return_ty,
        }
    }
}
