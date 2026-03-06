use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::io::{self, Write};

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

    async fn execute(&self, args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        if !args.is_empty() {
            return Err(format!("receive expects 0 arguments, got {}", args.len()));
        }

        print!("> ");
        io::stdout()
            .flush()
            .map_err(|e| format!("Failed to flush stdout: {}", e))?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read input: {}", e))?;

        let trimmed = input.trim().to_string();
        Ok(ExpressionValue::String(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_receive_function_properties() {
        let receive_fn = ReceiveFunction::new();

        assert_eq!(receive_fn.name(), "receive");
        assert_eq!(receive_fn.parameters().len(), 0);
        assert_eq!(receive_fn.return_type().name(), "String");
    }

    #[tokio::test]
    async fn test_receive_function_wrong_args_count() {
        let receive_fn = ReceiveFunction::new();

        let result = receive_fn
            .execute(vec![ExpressionValue::String("unexpected".to_string())])
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("receive expects 0 arguments, got 1")
        );
    }

    #[tokio::test]
    async fn test_receive_function_debug() {
        let receive_fn = ReceiveFunction::new();
        let debug_output = format!("{:?}", receive_fn);
        assert!(debug_output.contains("ReceiveFunction"));
    }
}
