use structured_agent_macros::sa_module;

#[sa_module]
pub mod messaging {
    use structured_agent_runtime::AgentMessageContent;

    #[sa_fn]
    async fn receive() -> String {
        let (msg, ack) = agent.recv_message().await?;
        let content = match &msg.content {
            AgentMessageContent::String(s) => s.clone(),
            _ => return Err("Unexpected message type in receive".to_string()),
        };
        ack.send(()).ok();
        content
    }

    #[sa_fn]
    async fn try_receive() -> String {
        match agent.try_recv_message().await {
            Some((msg, ack)) => {
                let content = match &msg.content {
                    AgentMessageContent::String(s) => s.clone(),
                    _ => return Err("Unexpected message type in try_receive".to_string()),
                };
                ack.send(()).ok();
                content
            }
            None => "No prompt received".to_string(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use structured_agent_runtime::{AgentHandle, ExpressionValue, Module, NativeFunction};

        #[tokio::test]
        async fn test_receive_properties() {
            let f = ReceiveFunction::new();
            assert_eq!(f.name(), "receive");
            assert_eq!(f.parameters().len(), 0);
            assert_eq!(f.return_type().name(), "String");
        }

        #[tokio::test]
        async fn test_receive_wrong_args_count() {
            let f = ReceiveFunction::new();
            let result = f
                .execute(vec![ExpressionValue::string("x")], &AgentHandle::detached())
                .await;
            assert!(result.is_err());
        }

        #[test]
        fn test_receive_default() {
            let f = ReceiveFunction::default();
            assert_eq!(f.name(), "receive");
        }

        #[tokio::test]
        async fn test_try_receive_properties() {
            let f = TryReceiveFunction::new();
            assert_eq!(f.name(), "try_receive");
            assert_eq!(f.parameters().len(), 0);
            assert_eq!(f.return_type().name(), "String");
        }

        #[tokio::test]
        async fn test_try_receive_channel_empty() {
            let f = TryReceiveFunction::new();
            let result = f.execute(vec![], &AgentHandle::detached()).await.unwrap();
            assert_eq!(result, ExpressionValue::string("No prompt received"));
        }

        #[test]
        fn test_try_receive_default() {
            let f = TryReceiveFunction::default();
            assert_eq!(f.name(), "try_receive");
        }

        #[tokio::test]
        async fn test_messaging_module_functions() {
            let module = MessagingModule;
            let fns = module.functions();
            assert_eq!(fns.len(), 2);
            let names: Vec<_> = fns.iter().map(|f| f.name().to_string()).collect();
            assert!(names.contains(&"receive".to_string()));
            assert!(names.contains(&"try_receive".to_string()));
        }
    }
}

pub use messaging::MessagingModule;
