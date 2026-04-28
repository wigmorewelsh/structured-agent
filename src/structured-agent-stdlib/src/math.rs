use structured_agent_macros::sa_module;

#[sa_module]
pub mod math {
    #[allow(unused_imports)]
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

    #[sa_trait]
    trait Modulo {
        fn modulo(self: Self, other: Self) -> Self;
    }

    #[sa_impl]
    impl Modulo for IntValue {
        fn modulo(&self, other: IntValue) -> IntValue {
            if *other == 0 {
                panic!("modulo by zero");
            }
            (*self % *other).into()
        }
    }

    #[sa_trait]
    trait Negate {
        fn negate(self: Self) -> Self;
    }

    #[sa_impl]
    impl Negate for IntValue {
        fn negate(&self) -> IntValue {
            (-*self).into()
        }
    }

    #[sa_trait]
    trait Abs {
        fn abs(self: Self) -> Self;
    }

    #[sa_impl]
    impl Abs for IntValue {
        fn abs(&self) -> IntValue {
            (*self).abs().into()
        }
    }

    #[sa_trait]
    trait Power {
        fn power(self: Self, exp: Self) -> Self;
    }

    #[sa_impl]
    impl Power for IntValue {
        fn power(&self, exp: IntValue) -> IntValue {
            if *exp < 0 {
                panic!("negative exponent");
            }
            (*self).pow(*exp as u32).into()
        }
    }

    #[sa_trait]
    trait Min {
        fn min(self: Self, other: Self) -> Self;
    }

    #[sa_impl]
    impl Min for IntValue {
        fn min(&self, other: IntValue) -> IntValue {
            (*self).min(*other).into()
        }
    }

    #[sa_trait]
    trait Max {
        fn max(self: Self, other: Self) -> Self;
    }

    #[sa_impl]
    impl Max for IntValue {
        fn max(&self, other: IntValue) -> IntValue {
            (*self).max(*other).into()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            int_abs_impl, int_add_impl, int_divide_impl, int_max_impl, int_min_impl,
            int_modulo_impl, int_multiply_impl, int_negate_impl, int_power_impl, int_subtract_impl,
        };
        use structured_agent_il::Instruction;
        use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFnPtr};

        fn get_fn_ptr(def: &structured_agent_il::NativeFunctionDef) -> NativeFnPtr {
            if let Instruction::CallNative { f, .. } = &def.body[0] {
                f.clone()
            } else {
                panic!("expected CallNative instruction");
            }
        }

        #[tokio::test]
        async fn test_add() {
            let f = get_fn_ptr(&int_add_impl::add_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(3), ExpressionValue::integer(4)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 7);
        }

        #[tokio::test]
        async fn test_subtract() {
            let f = get_fn_ptr(&int_subtract_impl::subtract_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(10), ExpressionValue::integer(3)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 7);
        }

        #[tokio::test]
        async fn test_multiply() {
            let f = get_fn_ptr(&int_multiply_impl::multiply_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(3), ExpressionValue::integer(4)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 12);
        }

        #[tokio::test]
        async fn test_divide() {
            let f = get_fn_ptr(&int_divide_impl::divide_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(12), ExpressionValue::integer(4)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 3);
        }

        #[tokio::test]
        #[should_panic(expected = "division by zero")]
        async fn test_divide_by_zero() {
            let f = get_fn_ptr(&int_divide_impl::divide_native_def());
            f.call(
                vec![ExpressionValue::integer(5), ExpressionValue::integer(0)],
                AgentHandle::detached(),
            )
            .await
            .unwrap();
        }

        #[tokio::test]
        async fn test_modulo() {
            let f = get_fn_ptr(&int_modulo_impl::modulo_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(10), ExpressionValue::integer(3)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 1);
        }

        #[tokio::test]
        #[should_panic(expected = "modulo by zero")]
        async fn test_modulo_by_zero() {
            let f = get_fn_ptr(&int_modulo_impl::modulo_native_def());
            f.call(
                vec![ExpressionValue::integer(5), ExpressionValue::integer(0)],
                AgentHandle::detached(),
            )
            .await
            .unwrap();
        }

        #[tokio::test]
        async fn test_negate_positive() {
            let f = get_fn_ptr(&int_negate_impl::negate_native_def());
            let result = f
                .call(vec![ExpressionValue::integer(5)], AgentHandle::detached())
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), -5);
        }

        #[tokio::test]
        async fn test_negate_negative() {
            let f = get_fn_ptr(&int_negate_impl::negate_native_def());
            let result = f
                .call(vec![ExpressionValue::integer(-5)], AgentHandle::detached())
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 5);
        }

        #[tokio::test]
        async fn test_abs_positive() {
            let f = get_fn_ptr(&int_abs_impl::abs_native_def());
            let result = f
                .call(vec![ExpressionValue::integer(5)], AgentHandle::detached())
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 5);
        }

        #[tokio::test]
        async fn test_abs_negative() {
            let f = get_fn_ptr(&int_abs_impl::abs_native_def());
            let result = f
                .call(vec![ExpressionValue::integer(-5)], AgentHandle::detached())
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 5);
        }

        #[tokio::test]
        async fn test_power() {
            let f = get_fn_ptr(&int_power_impl::power_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(2), ExpressionValue::integer(10)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 1024);
        }

        #[tokio::test]
        async fn test_power_zero_exp() {
            let f = get_fn_ptr(&int_power_impl::power_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(5), ExpressionValue::integer(0)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 1);
        }

        #[tokio::test]
        #[should_panic(expected = "negative exponent")]
        async fn test_power_negative_exp() {
            let f = get_fn_ptr(&int_power_impl::power_native_def());
            f.call(
                vec![ExpressionValue::integer(2), ExpressionValue::integer(-1)],
                AgentHandle::detached(),
            )
            .await
            .unwrap();
        }

        #[tokio::test]
        async fn test_min_returns_smaller() {
            let f = get_fn_ptr(&int_min_impl::min_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(3), ExpressionValue::integer(7)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 3);
        }

        #[tokio::test]
        async fn test_min_equal_values() {
            let f = get_fn_ptr(&int_min_impl::min_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(5), ExpressionValue::integer(5)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 5);
        }

        #[tokio::test]
        async fn test_max_returns_larger() {
            let f = get_fn_ptr(&int_max_impl::max_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(3), ExpressionValue::integer(7)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 7);
        }

        #[tokio::test]
        async fn test_max_equal_values() {
            let f = get_fn_ptr(&int_max_impl::max_native_def());
            let result = f
                .call(
                    vec![ExpressionValue::integer(5), ExpressionValue::integer(5)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_integer().unwrap(), 5);
        }
    }
}

pub use math::MathModule;
