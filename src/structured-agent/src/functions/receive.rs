use crate::runtime::{AgentHandle, AgentMessageContent, ExpressionValue};
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct ReceiveFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for ReceiveFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceiveFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![],
            return_type: Type::string(),
        }
    }
}

#[async_trait]
impl NativeFunction for ReceiveFunction {
    fn name(&self) -> &str {
        "receive"
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
            return Err(format!("receive expects 0 arguments, got {}", args.len()));
        }
        let (msg, ack) = agent.recv_message().await?;
        let content = match &msg.content {
            AgentMessageContent::String(s) => s.clone(),
            _ => return Err("Unexpected message type in receive".to_string()),
        };
        ack.send(()).ok();
        Ok(ExpressionValue::string(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_receive_function_properties() {
        let f = ReceiveFunction::new();
        assert_eq!(f.name(), "receive");
        assert_eq!(f.parameters().len(), 0);
        assert_eq!(f.return_type().name(), "String");
    }

    #[tokio::test]
    async fn test_receive_function_wrong_args_count() {
        let f = ReceiveFunction::new();
        let result = f
            .execute(vec![ExpressionValue::string("x")], &AgentHandle::detached())
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("receive expects 0 arguments, got 1")
        );
    }

    #[test]
    fn test_receive_function_debug() {
        let f = ReceiveFunction::new();
        let debug = format!("{:?}", f);
        assert!(debug.contains("ReceiveFunction"));
    }
}
