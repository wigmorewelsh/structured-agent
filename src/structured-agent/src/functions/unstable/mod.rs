pub mod head;
pub mod option;
pub mod tail;

pub use head::HeadFunction;
pub use option::{IsSomeFunction, IsSomeListFunction, SomeValueFunction, SomeValueListFunction};
pub use tail::TailFunction;
