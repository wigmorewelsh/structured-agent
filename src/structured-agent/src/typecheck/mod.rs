pub mod checker;
mod error;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_test;

pub use checker::TypeChecker;
pub use error::TypeError;
