use crate::runtime::{AgentHandle, AgentMessageContent, ExpressionValue};
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct PrintFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for PrintFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl PrintFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new("value".to_string(), Type::string())],
            return_type: Type::unit(),
        }
    }
}

#[async_trait]
impl NativeFunction for PrintFunction {
    fn name(&self) -> &str {
        "print"
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
        if args.len() != 1 {
            return Err(format!("print expects 1 argument, got {}", args.len()));
        }
        let value = args[0].format_for_llm();
        agent.publish(AgentMessageContent::String(value));
        Ok(ExpressionValue::unit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_print_function_properties() {
        let print_fn = PrintFunction::new();
        assert_eq!(print_fn.name(), "print");
        assert_eq!(print_fn.parameters().len(), 1);
        assert_eq!(print_fn.parameters()[0].name, "value");
        assert_eq!(print_fn.parameters()[0].param_type.name(), "String");
        assert_eq!(print_fn.return_type().name(), "()");
    }

    #[tokio::test]
    async fn test_print_function_execute_string() {
        let print_fn = PrintFunction::new();
        let args = vec![ExpressionValue::string("Hello, World!")];
        let result = print_fn
            .execute(args, &AgentHandle::detached())
            .await
            .unwrap();
        assert_eq!(result, ExpressionValue::unit());
    }

    #[tokio::test]
    async fn test_print_function_execute_boolean() {
        let print_fn = PrintFunction::new();
        let args = vec![ExpressionValue::boolean(true)];
        let result = print_fn
            .execute(args, &AgentHandle::detached())
            .await
            .unwrap();
        assert_eq!(result, ExpressionValue::unit());
    }

    #[tokio::test]
    async fn test_print_function_execute_unit() {
        let print_fn = PrintFunction::new();
        let args = vec![ExpressionValue::unit()];
        let result = print_fn
            .execute(args, &AgentHandle::detached())
            .await
            .unwrap();
        assert_eq!(result, ExpressionValue::unit());
    }

    #[tokio::test]
    async fn test_print_function_wrong_args_count() {
        let print_fn = PrintFunction::new();
        let handle = AgentHandle::detached();

        let result = print_fn.execute(vec![], &handle).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("print expects 1 argument, got 0")
        );

        let result = print_fn
            .execute(
                vec![ExpressionValue::string("a"), ExpressionValue::string("b")],
                &handle,
            )
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("print expects 1 argument, got 2")
        );
    }
}
