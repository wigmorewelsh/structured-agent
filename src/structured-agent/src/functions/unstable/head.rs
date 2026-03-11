use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use arrow::array::{Array, Datum, StringArray};
use async_trait::async_trait;

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

    async fn execute(&self, args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        if args.len() != 1 {
            return Err(format!("head expects 1 argument, got {}", args.len()));
        }

        let list = args[0]
            .as_list()
            .map_err(|_| "head expects a list argument")?;

        if list.len() == 0 {
            return Ok(ExpressionValue::option_none());
        }

        let values = list.value(0);
        if values.len() == 0 {
            return Ok(ExpressionValue::option_none());
        }

        let string_array = values
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("Expected string array")?;

        Ok(ExpressionValue::option_some(ExpressionValue::string(
            string_array.value(0),
        )))
    }

    fn documentation(&self) -> Option<&str> {
        Some("Returns the first element of a list as Option, or None if the list is empty")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{ListBuilder, StringBuilder};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_head_function_properties() {
        let head_fn = HeadFunction::new();

        assert_eq!(head_fn.name(), "head");
        assert_eq!(head_fn.parameters().len(), 1);
        assert_eq!(head_fn.parameters()[0].name, "list");
        assert_eq!(head_fn.return_type().name(), "Option<String>");
    }

    #[tokio::test]
    async fn test_head_function_with_non_empty_list() {
        let head_fn = HeadFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("first");
        builder.values().append_value("second");
        builder.append(true);

        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];

        let result = head_fn.execute(args).await.unwrap();
        let inner = result.as_option().unwrap().unwrap();
        assert_eq!(inner.as_string().unwrap(), "first");
    }

    #[tokio::test]
    async fn test_head_function_with_empty_list() {
        let head_fn = HeadFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];

        let result = head_fn.execute(args).await.unwrap();
        assert!(result.as_option().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_head_function_wrong_argument_type() {
        let head_fn = HeadFunction::new();
        let args = vec![ExpressionValue::string("not a list")];

        let result = head_fn.execute(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("head expects a list argument"));
    }

    #[tokio::test]
    async fn test_head_function_wrong_args_count() {
        let head_fn = HeadFunction::new();

        let result = head_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("head expects 1 argument, got 0")
        );
    }
}
