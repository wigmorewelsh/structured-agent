use arrow::array::Array;
use async_trait::async_trait;
use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFunction, Parameter, Type};

#[derive(Debug)]
pub struct HeadFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for HeadFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new(
                "list".to_string(),
                Type::list(Type::string()),
            )],
            return_type: Type::option(Type::string()),
        }
    }
}

#[async_trait]
impl NativeFunction for HeadFunction {
    fn name(&self) -> &str {
        "head"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    fn documentation(&self) -> Option<&str> {
        Some("Returns the first element of a list as Option, or None if the list is empty")
    }

    async fn execute(
        &self,
        args: Vec<ExpressionValue>,
        _agent: &AgentHandle,
    ) -> Result<ExpressionValue, String> {
        if args.len() != 1 {
            return Err(format!("head expects 1 argument, got {}", args.len()));
        }

        let list = args[0]
            .as_list()
            .map_err(|_| "head expects a list argument")?;

        if list.is_empty() {
            return Ok(ExpressionValue::option_none());
        }

        let values = list.value(0);
        if values.is_empty() {
            return Ok(ExpressionValue::option_none());
        }

        let first = values.slice(0, 1);
        Ok(ExpressionValue::option_some(ExpressionValue::from_array(
            first,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{ListBuilder, StringBuilder};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_head_function_properties() {
        let f = HeadFunction::new();
        assert_eq!(f.name(), "head");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(f.parameters()[0].name, "list");
        assert_eq!(f.return_type().name(), "Option<String>");
    }

    #[tokio::test]
    async fn test_head_function_with_non_empty_list() {
        let f = HeadFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("first");
        builder.values().append_value("second");
        builder.append(true);

        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];

        let result = f.execute(args, &AgentHandle::detached()).await.unwrap();
        let inner = result.as_option().unwrap().unwrap();
        assert_eq!(inner.as_string().unwrap(), "first");
    }

    #[tokio::test]
    async fn test_head_function_with_empty_list() {
        let f = HeadFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];

        let result = f.execute(args, &AgentHandle::detached()).await.unwrap();
        assert!(result.as_option().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_head_function_wrong_argument_type() {
        let f = HeadFunction::new();
        let args = vec![ExpressionValue::string("not a list")];

        let result = f.execute(args, &AgentHandle::detached()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("head expects a list argument"));
    }

    #[tokio::test]
    async fn test_head_function_wrong_args_count() {
        let f = HeadFunction::new();

        let result = f.execute(vec![], &AgentHandle::detached()).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("head expects 1 argument, got 0")
        );
    }
}
