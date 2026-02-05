use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Parameter, Type};
use arrow::array::{Array, ListBuilder, StringBuilder};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug)]
pub struct TailFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for TailFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl TailFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new(
                "list".to_string(),
                Type::list(Type::string()),
            )],
            return_type: Type::option(Type::list(Type::string())),
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for TailFunction {
    fn name(&self) -> &str {
        "tail"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
        if args.len() != 1 {
            return Err(format!("tail expects 1 argument, got {}", args.len()));
        }

        match &args[0] {
            ExprResult::List(list) => {
                if list.len() == 0 {
                    Ok(ExprResult::Option(None))
                } else {
                    let values = list.value(0);
                    if values.len() <= 1 {
                        let mut builder = ListBuilder::new(StringBuilder::new());
                        builder.append(true);
                        let empty_list = Arc::new(builder.finish());
                        Ok(ExprResult::Option(Some(Box::new(ExprResult::List(
                            empty_list,
                        )))))
                    } else {
                        let string_array = values
                            .as_any()
                            .downcast_ref::<arrow::array::StringArray>()
                            .ok_or("Expected string array")?;

                        let mut builder = ListBuilder::new(StringBuilder::new());
                        let values_builder = builder.values();

                        for i in 1..string_array.len() {
                            values_builder.append_value(string_array.value(i));
                        }
                        builder.append(true);

                        let tail_list = Arc::new(builder.finish());
                        Ok(ExprResult::Option(Some(Box::new(ExprResult::List(
                            tail_list,
                        )))))
                    }
                }
            }
            _ => Err("tail expects a list argument".to_string()),
        }
    }

    fn documentation(&self) -> Option<&str> {
        Some("Returns all elements except the first as Option<List>, or None if the list is empty")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{ListBuilder, StringBuilder};

    #[tokio::test]
    async fn test_tail_function_properties() {
        let tail_fn = TailFunction::new();

        assert_eq!(tail_fn.name(), "tail");
        assert_eq!(tail_fn.parameters().len(), 1);
        assert_eq!(tail_fn.parameters()[0].name, "list");
        assert_eq!(tail_fn.return_type().name(), "Option<List<String>>");
    }

    #[tokio::test]
    async fn test_tail_function_with_multiple_elements() {
        let tail_fn = TailFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        let values = builder.values();
        values.append_value("first");
        values.append_value("second");
        values.append_value("third");
        builder.append(true);

        let list_array = Arc::new(builder.finish());
        let args = vec![ExprResult::List(list_array)];

        let result = tail_fn.execute(args).await.unwrap();
        match result {
            ExprResult::Option(Some(inner)) => match *inner {
                ExprResult::List(tail_list) => {
                    assert_eq!(tail_list.len(), 1);
                    let tail_values = tail_list.value(0);
                    assert_eq!(tail_values.len(), 2);
                }
                _ => panic!("Expected List inside Some"),
            },
            _ => panic!("Expected Some result"),
        }
    }

    #[tokio::test]
    async fn test_tail_function_with_single_element() {
        let tail_fn = TailFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        let values = builder.values();
        values.append_value("only");
        builder.append(true);

        let list_array = Arc::new(builder.finish());
        let args = vec![ExprResult::List(list_array)];

        let result = tail_fn.execute(args).await.unwrap();
        match result {
            ExprResult::Option(Some(inner)) => match *inner {
                ExprResult::List(tail_list) => {
                    assert_eq!(tail_list.len(), 1);
                    let tail_values = tail_list.value(0);
                    assert_eq!(tail_values.len(), 0);
                }
                _ => panic!("Expected List inside Some"),
            },
            _ => panic!("Expected Some result"),
        }
    }

    #[tokio::test]
    async fn test_tail_function_with_empty_list() {
        let tail_fn = TailFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        let list_array = Arc::new(builder.finish());
        let args = vec![ExprResult::List(list_array)];

        let result = tail_fn.execute(args).await.unwrap();
        assert!(matches!(result, ExprResult::Option(None)));
    }

    #[tokio::test]
    async fn test_tail_function_wrong_argument_type() {
        let tail_fn = TailFunction::new();
        let args = vec![ExprResult::String("not a list".to_string())];

        let result = tail_fn.execute(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("tail expects a list argument"));
    }

    #[tokio::test]
    async fn test_tail_function_wrong_args_count() {
        let tail_fn = TailFunction::new();

        let result = tail_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("tail expects 1 argument, got 0")
        );
    }
}
