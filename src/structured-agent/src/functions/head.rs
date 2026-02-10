use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use arrow::array::Array;
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

#[async_trait(?Send)]
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

        match &args[0] {
            ExpressionValue::List(list) => {
                if list.len() == 0 {
                    Ok(ExpressionValue::Option(None))
                } else {
                    let values = list.value(0);
                    if values.len() == 0 {
                        Ok(ExpressionValue::Option(None))
                    } else {
                        let string_array = values
                            .as_any()
                            .downcast_ref::<arrow::array::StringArray>()
                            .ok_or("Expected string array")?;
                        let first_value = string_array.value(0);
                        Ok(ExpressionValue::Option(Some(Box::new(
                            ExpressionValue::String(first_value.to_string()),
                        ))))
                    }
                }
            }
            _ => Err("head expects a list argument".to_string()),
        }
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
        let values = builder.values();
        values.append_value("first");
        values.append_value("second");
        builder.append(true);

        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::List(list_array)];

        let result = head_fn.execute(args).await.unwrap();
        match result {
            ExpressionValue::Option(Some(inner)) => match *inner {
                ExpressionValue::String(s) => assert_eq!(s, "first"),
                _ => panic!("Expected String inside Some"),
            },
            _ => panic!("Expected Some result"),
        }
    }

    #[tokio::test]
    async fn test_head_function_with_empty_list() {
        let head_fn = HeadFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::List(list_array)];

        let result = head_fn.execute(args).await.unwrap();
        assert!(matches!(result, ExpressionValue::Option(None)));
    }

    #[tokio::test]
    async fn test_head_function_wrong_argument_type() {
        let head_fn = HeadFunction::new();
        let args = vec![ExpressionValue::String("not a list".to_string())];

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
