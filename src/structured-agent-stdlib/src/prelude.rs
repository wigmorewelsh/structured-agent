use structured_agent_macros::sa_module;

#[sa_module]
pub mod prelude {
    #[sa_trait]
    trait ToString {
        fn to_string(self: Self) -> StringValue;
    }

    #[sa_impl(Int for ToString)]
    mod int_impl {
        use structured_agent_runtime::{IntValue, StringValue};

        #[sa_fn]
        fn to_string(value: IntValue) -> StringValue {
            value.to_string().into()
        }
    }

    #[sa_impl(String for ToString)]
    mod string_impl {
        use structured_agent_runtime::StringValue;

        #[sa_fn]
        fn to_string(value: StringValue) -> StringValue {
            value
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{int_impl, string_impl};
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
        async fn test_int_to_string() {
            let def = int_impl::to_string_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(vec![ExpressionValue::integer(42)], AgentHandle::detached())
                .await
                .unwrap();
            assert_eq!(result.as_string().unwrap(), "42");
        }

        #[tokio::test]
        async fn test_string_to_string() {
            let def = string_impl::to_string_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::string("hello")],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result.as_string().unwrap(), "hello");
        }
    }
}

pub use prelude::PreludeModule;
