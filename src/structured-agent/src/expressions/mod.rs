pub mod assignment;
pub mod call;
pub mod function;
pub mod injection;
pub mod string_literal;
pub mod variable;

pub use assignment::AssignmentExpr;
pub use call::CallExpr;
pub use function::FunctionExpr;
pub use injection::InjectionExpr;
pub use string_literal::StringLiteralExpr;
pub use variable::VariableExpr;

pub use crate::types::Expression;
