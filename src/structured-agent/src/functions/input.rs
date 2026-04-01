use crate::runtime::{AgentHandle, ExpressionValue};
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct InputFunction {
    return_type: Type,
}

impl Default for InputFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl InputFunction {
    pub fn new() -> Self {
        Self {
            return_type: Type::string(),
        }
    }
}

#[async_trait]
impl NativeFunction for InputFunction {
    fn name(&self) -> &str {
        "input"
    }

    fn parameters(&self) -> &[Parameter] {
        &[]
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(
        &self,
        _args: Vec<ExpressionValue>,
        agent: &AgentHandle,
    ) -> Result<ExpressionValue, String> {
        let response = agent.publish_input_request(String::new()).await?;
        Ok(ExpressionValue::string(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_input_function_properties() {
        let input_fn = InputFunction::new();

        assert_eq!(input_fn.name(), "input");
        assert_eq!(input_fn.parameters().len(), 0);
        assert_eq!(input_fn.return_type().name(), "String");
    }

    #[tokio::test]
    async fn test_input_function_debug() {
        let input_fn = InputFunction::new();
        let debug_output = format!("{:?}", input_fn);
        assert!(debug_output.contains("InputFunction"));
    }
}
