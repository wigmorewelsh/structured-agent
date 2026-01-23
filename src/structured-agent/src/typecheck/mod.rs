mod checker;
mod error;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_test;

pub use checker::TypeChecker;
pub use error::TypeError;

use crate::ast::Module;

pub fn type_check_module(module: &Module) -> Result<(), TypeError> {
    let mut checker = TypeChecker::new();
    checker.check_module(module)
}
