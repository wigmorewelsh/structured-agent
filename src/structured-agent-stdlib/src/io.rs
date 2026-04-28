use structured_agent_macros::sa_module;

#[sa_module]
pub mod io {
    use structured_agent_runtime::AgentMessageContent;
    use structured_agent_runtime::StringValue;

    #[sa_fn]
    async fn print(value: StringValue) {
        agent.publish(AgentMessageContent::String(value.into()));
    }

    #[sa_fn]
    async fn input() -> StringValue {
        agent.publish_input_request(String::new()).await?.into()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use structured_agent_il::Instruction;
        use structured_agent_runtime::{AgentHandle, ExpressionValue};

        fn get_fn_ptr(
            def: &structured_agent_il::NativeFunctionDef,
        ) -> structured_agent_runtime::NativeFnPtr {
            if let Instruction::CallNative { f, .. } = &def.body[0] {
                f.clone()
            } else {
                panic!("expected CallNative instruction");
            }
        }

        #[tokio::test]
        async fn test_print_properties() {
            let def = print_native_def();
            assert_eq!(def.name, "print");
            assert_eq!(def.parameters.len(), 1);
            assert_eq!(def.parameters[0].name, "value");
        }

        #[tokio::test]
        async fn test_print_execute() {
            let def = print_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::string("hello")],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result, ExpressionValue::unit());
        }

        #[tokio::test]
        async fn test_print_wrong_args() {
            let def = print_native_def();
            let f = get_fn_ptr(&def);
            let result = f.call(vec![], AgentHandle::detached()).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_input_properties() {
            let def = input_native_def();
            assert_eq!(def.name, "input");
            assert_eq!(def.parameters.len(), 0);
            assert_eq!(def.return_type.name(), "String");
        }
    }
}

pub use io::IoModule;
