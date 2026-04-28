use structured_agent_macros::sa_module;

#[sa_module]
pub mod messaging {
    use structured_agent_runtime::AgentMessageContent;
    use structured_agent_runtime::StringValue;

    #[sa_fn]
    async fn receive() -> StringValue {
        let (msg, ack) = agent.recv_message().await?;
        let content = match &msg.content {
            AgentMessageContent::String(s) => s.clone(),
            _ => return Err("Unexpected message type in receive".to_string()),
        };
        ack.send(()).ok();
        content.into()
    }

    #[sa_fn]
    async fn try_receive() -> StringValue {
        match agent.try_recv_message().await {
            Some((msg, ack)) => {
                let content = match &msg.content {
                    AgentMessageContent::String(s) => s.clone(),
                    _ => return Err("Unexpected message type in try_receive".to_string()),
                };
                ack.send(()).ok();
                content.into()
            }
            None => "No prompt received".into(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use structured_agent_il::{Instruction, Module};
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
        async fn test_receive_properties() {
            let def = receive_native_def();
            assert_eq!(def.name, "receive");
            assert_eq!(def.parameters.len(), 0);
            assert_eq!(def.return_type.name(), "String");
        }

        #[tokio::test]
        async fn test_receive_wrong_args_count() {
            let def = receive_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(vec![ExpressionValue::string("x")], AgentHandle::detached())
                .await;
            assert!(result.is_err());
        }

        #[test]
        fn test_receive_def_name() {
            let def = receive_native_def();
            assert_eq!(def.name, "receive");
        }

        #[tokio::test]
        async fn test_try_receive_properties() {
            let def = try_receive_native_def();
            assert_eq!(def.name, "try_receive");
            assert_eq!(def.parameters.len(), 0);
            assert_eq!(def.return_type.name(), "String");
        }

        #[tokio::test]
        async fn test_try_receive_channel_empty() {
            let def = try_receive_native_def();
            let f = get_fn_ptr(&def);
            let result = f.call(vec![], AgentHandle::detached()).await.unwrap();
            assert_eq!(result, ExpressionValue::string("No prompt received"));
        }

        #[test]
        fn test_try_receive_def_name() {
            let def = try_receive_native_def();
            assert_eq!(def.name, "try_receive");
        }

        #[tokio::test]
        async fn test_messaging_module_functions() {
            let module = MessagingModule;
            let fns = module.native_functions();
            assert_eq!(fns.len(), 2);
            let names: Vec<_> = fns.iter().map(|def| def.name.clone()).collect();
            assert!(names.contains(&"receive".to_string()));
            assert!(names.contains(&"try_receive".to_string()));
        }
    }
}

pub use messaging::MessagingModule;
