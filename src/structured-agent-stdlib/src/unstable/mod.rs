pub mod head;
pub mod option;
pub mod tail;

pub use head::HeadFunction;
pub use option::{IsSomeFunction, SomeValueFunction};
pub use tail::TailFunction;

pub struct UnstableModule;

impl ::structured_agent_runtime::Module for UnstableModule {
    fn name(&self) -> &str {
        "unstable"
    }

    fn functions(&self) -> Vec<::std::sync::Arc<dyn ::structured_agent_runtime::NativeFunction>> {
        vec![
            ::std::sync::Arc::new(HeadFunction::new()),
            ::std::sync::Arc::new(TailFunction::new()),
            ::std::sync::Arc::new(IsSomeFunction::for_string()),
            ::std::sync::Arc::new(SomeValueFunction::for_string()),
            ::std::sync::Arc::new(IsSomeFunction::for_list()),
            ::std::sync::Arc::new(SomeValueFunction::for_list()),
        ]
    }
}
