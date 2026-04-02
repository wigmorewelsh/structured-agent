use structured_agent_macros::sa_module;

#[sa_module]
pub mod io {
    use structured_agent_runtime::AgentMessageContent;

    #[sa_fn]
    async fn print(value: String) {
        agent.publish(AgentMessageContent::String(value));
    }

    #[sa_fn]
    async fn input() -> String {
        agent.publish_input_request(String::new()).await?
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFunction};

        #[tokio::test]
        async fn test_print_properties() {
            let f = PrintFunction::new();
            assert_eq!(f.name(), "print");
            assert_eq!(f.parameters().len(), 1);
            assert_eq!(f.parameters()[0].name, "value");
        }

        #[tokio::test]
        async fn test_print_execute() {
            let f = PrintFunction::new();
            let result = f
                .execute(
                    vec![ExpressionValue::string("hello")],
                    &AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result, ExpressionValue::unit());
        }

        #[tokio::test]
        async fn test_print_wrong_args() {
            let f = PrintFunction::new();
            let result = f.execute(vec![], &AgentHandle::detached()).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_input_properties() {
            let f = InputFunction::new();
            assert_eq!(f.name(), "input");
            assert_eq!(f.parameters().len(), 0);
            assert_eq!(f.return_type().name(), "String");
        }
    }
}

pub use io::IoModule;
