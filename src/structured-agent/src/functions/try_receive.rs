use crate::runtime::{AgentHandle, AgentMessageContent, ExpressionValue};
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct TryReceiveFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for TryReceiveFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl TryReceiveFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![],
            return_type: Type::string(),
        }
    }
}

#[async_trait]
impl NativeFunction for TryReceiveFunction {
    fn name(&self) -> &str {
        "try_receive"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(
        &self,
        args: Vec<ExpressionValue>,
        agent: &AgentHandle,
    ) -> Result<ExpressionValue, String> {
        if !args.is_empty() {
            return Err(format!(
                "try_receive expects 0 arguments, got {}",
                args.len()
            ));
        }
        match agent.try_recv_message().await {
            Some((msg, ack)) => {
                let content = match &msg.content {
                    AgentMessageContent::String(s) => s.clone(),
                    _ => return Err("Unexpected message type in try_receive".to_string()),
                };
                ack.send(()).ok();
                Ok(ExpressionValue::string(content))
            }
            None => Ok(ExpressionValue::string("No prompt received")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_try_receive_function_properties() {
        let f = TryReceiveFunction::new();
        assert_eq!(f.name(), "try_receive");
        assert_eq!(f.parameters().len(), 0);
        assert_eq!(f.return_type().name(), "String");
    }

    #[tokio::test]
    async fn test_try_receive_function_wrong_args_count() {
        let f = TryReceiveFunction::new();
        let result = f
            .execute(vec![ExpressionValue::string("x")], &AgentHandle::detached())
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("try_receive expects 0 arguments, got 1")
        );
    }

    #[test]
    fn test_try_receive_function_debug() {
        let f = TryReceiveFunction::new();
        let debug = format!("{:?}", f);
        assert!(debug.contains("TryReceiveFunction"));
    }

    #[tokio::test]
    async fn test_try_receive_function_channel_empty() {
        let f = TryReceiveFunction::new();
        let result = f.execute(vec![], &AgentHandle::detached()).await.unwrap();
        assert_eq!(result, ExpressionValue::string("No prompt received"));
    }
}
