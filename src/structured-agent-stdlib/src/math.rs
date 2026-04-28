use structured_agent_macros::sa_module;

#[sa_module]
pub mod math {
    use structured_agent_runtime::IntValue;

    #[sa_trait]
    trait Add {
        fn add(self: Self, other: Self) -> Self;
    }

    #[sa_impl]
    impl Add for IntValue {
        fn add(&self, other: IntValue) -> IntValue {
            (*self + *other).into()
        }
    }

    #[sa_trait]
    trait Subtract {
        fn subtract(self: Self, other: Self) -> Self;
    }

    #[sa_impl]
    impl Subtract for IntValue {
        fn subtract(&self, other: IntValue) -> IntValue {
            (*self - *other).into()
        }
    }

    #[sa_trait]
    trait Multiply {
        fn multiply(self: Self, other: Self) -> Self;
    }

    #[sa_impl]
    impl Multiply for IntValue {
        fn multiply(&self, other: IntValue) -> IntValue {
            (*self * *other).into()
        }
    }

    #[sa_trait]
    trait Divide {
        fn divide(self: Self, other: Self) -> Self;
    }

    #[sa_impl]
    impl Divide for IntValue {
        fn divide(&self, other: IntValue) -> IntValue {
            if *other == 0 {
                panic!("division by zero");
            }
            (*self / *other).into()
        }
    }
}
