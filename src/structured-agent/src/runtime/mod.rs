mod context;
mod engine;

#[cfg(test)]
mod scoping_test;

#[cfg(test)]
mod function_call_test;

#[cfg(test)]
mod boolean_test;

#[cfg(test)]
mod control_flow_test;

pub use context::{Context, ExprResult};
pub use engine::{Runtime, load_program};
