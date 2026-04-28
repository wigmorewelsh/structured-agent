use structured_agent_macros::sa_module;

#[sa_module]
pub mod equality {
    #[allow(unused_imports)]
    use structured_agent_runtime::{BooleanValue, IntValue, StringValue};

    #[sa_trait]
    trait Equal {
        fn equal(self: Self, other: Self) -> BooleanValue;
    }

    #[sa_impl]
    impl Equal for IntValue {
        fn equal(&self, other: IntValue) -> BooleanValue {
            (*self == *other).into()
        }
    }

    #[sa_impl]
    impl Equal for StringValue {
        fn equal(&self, other: StringValue) -> BooleanValue {
            (*self == *other).into()
        }
    }

    #[sa_impl]
    impl Equal for BooleanValue {
        fn equal(&self, other: BooleanValue) -> BooleanValue {
            (*self == *other).into()
        }
    }

    #[sa_trait]
    trait Compare {
        fn less_than(self: Self, other: Self) -> BooleanValue;
        fn greater_than(self: Self, other: Self) -> BooleanValue;
    }

    #[sa_impl]
    impl Compare for IntValue {
        fn less_than(&self, other: IntValue) -> BooleanValue {
            (*self < *other).into()
        }

        fn greater_than(&self, other: IntValue) -> BooleanValue {
            (*self > *other).into()
        }
    }

    #[sa_impl]
    impl Compare for StringValue {
        fn less_than(&self, other: StringValue) -> BooleanValue {
            (*self < *other).into()
        }

        fn greater_than(&self, other: StringValue) -> BooleanValue {
            (*self > *other).into()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            boolean_equal_impl, int_compare_impl, int_equal_impl, string_compare_impl,
            string_equal_impl,
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
        async fn test_int_equal_same() {
            let def = int_equal_impl::equal_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::integer(5), ExpressionValue::integer(5)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_int_equal_different() {
            let def = int_equal_impl::equal_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::integer(5), ExpressionValue::integer(6)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(!result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_string_equal_same() {
            let def = string_equal_impl::equal_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![
                        ExpressionValue::string("hello"),
                        ExpressionValue::string("hello"),
                    ],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_string_equal_different() {
            let def = string_equal_impl::equal_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![
                        ExpressionValue::string("hello"),
                        ExpressionValue::string("world"),
                    ],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(!result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_boolean_equal_same() {
            let def = boolean_equal_impl::equal_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![
                        ExpressionValue::boolean(true),
                        ExpressionValue::boolean(true),
                    ],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_boolean_equal_different() {
            let def = boolean_equal_impl::equal_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![
                        ExpressionValue::boolean(true),
                        ExpressionValue::boolean(false),
                    ],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(!result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_int_less_than_true() {
            let def = int_compare_impl::less_than_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::integer(3), ExpressionValue::integer(7)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_int_less_than_false() {
            let def = int_compare_impl::less_than_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::integer(7), ExpressionValue::integer(3)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(!result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_int_greater_than_true() {
            let def = int_compare_impl::greater_than_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::integer(7), ExpressionValue::integer(3)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_int_greater_than_false() {
            let def = int_compare_impl::greater_than_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::integer(3), ExpressionValue::integer(7)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(!result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_string_less_than_true() {
            let def = string_compare_impl::less_than_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![
                        ExpressionValue::string("apple"),
                        ExpressionValue::string("banana"),
                    ],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(result.as_boolean().unwrap());
        }

        #[tokio::test]
        async fn test_string_greater_than_true() {
            let def = string_compare_impl::greater_than_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![
                        ExpressionValue::string("banana"),
                        ExpressionValue::string("apple"),
                    ],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert!(result.as_boolean().unwrap());
        }
    }
}

pub use equality::EqualityModule;
