use structured_agent_macros::sa_module;

#[sa_module]
pub mod logic {
    #[allow(unused_imports)]
    use structured_agent_runtime::BooleanValue;

    #[sa_trait]
    trait Logic {
        fn and(self: Self, other: Self) -> Self;
        fn or(self: Self, other: Self) -> Self;
    }

    #[sa_impl]
    impl Logic for BooleanValue {
        fn and(&self, other: BooleanValue) -> BooleanValue {
            (*self && *other).into()
        }

        fn or(&self, other: BooleanValue) -> BooleanValue {
            (*self || *other).into()
        }
    }
}

pub use logic::LogicModule;
