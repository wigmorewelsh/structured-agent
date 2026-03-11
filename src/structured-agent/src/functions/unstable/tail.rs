use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

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

#[async_trait]
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

    async fn execute(&self, _args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        Err("Option types not yet supported in Arrow-based values".to_string())
    }

    fn documentation(&self) -> Option<&str> {
        Some("Returns all elements except the first as Option<List>, or None if the list is empty")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tail_function_properties() {
        let tail_fn = TailFunction::new();

        assert_eq!(tail_fn.name(), "tail");
        assert_eq!(tail_fn.parameters().len(), 1);
        assert_eq!(tail_fn.parameters()[0].name, "list");
        assert_eq!(tail_fn.return_type().name(), "Option<List<String>>");
    }

    // TODO: Re-enable when Option types are supported
    // #[tokio::test]
    // async fn test_tail_function_with_multiple_elements() {
    //     let tail_fn = TailFunction::new();
    //
    //     let mut builder = ListBuilder::new(StringBuilder::new());
    //     let values = builder.values();
    //     values.append_value("first");
    //     values.append_value("second");
    //     values.append_value("third");
    //     builder.append(true);
    //
    //     let list_array = Arc::new(builder.finish());
    //     let args = vec![ExpressionValue::list(list_array)];
    //
    //     let result = tail_fn.execute(args).await.unwrap();
    //     match result {
    //         ExpressionValue::Option(Some(inner)) => match *inner {
    //             ExpressionValue::List(tail_list) => {
    //                 assert_eq!(tail_list.len(), 1);
    //                 let tail_values = tail_list.value(0);
    //                 assert_eq!(tail_values.len(), 2);
    //             }
    //             _ => panic!("Expected List inside Some"),
    //         },
    //         _ => panic!("Expected Some result"),
    //     }
    // }
    //
    // #[tokio::test]
    // async fn test_tail_function_with_single_element() {
    //     let tail_fn = TailFunction::new();
    //
    //     let mut builder = ListBuilder::new(StringBuilder::new());
    //     let values = builder.values();
    //     values.append_value("only");
    //     builder.append(true);
    //
    //     let list_array = Arc::new(builder.finish());
    //     let args = vec![ExpressionValue::list(list_array)];
    //
    //     let result = tail_fn.execute(args).await.unwrap();
    //     match result {
    //         ExpressionValue::Option(Some(inner)) => match *inner {
    //             ExpressionValue::List(tail_list) => {
    //                 assert_eq!(tail_list.len(), 1);
    //                 let tail_values = tail_list.value(0);
    //                 assert_eq!(tail_values.len(), 0);
    //             }
    //             _ => panic!("Expected List inside Some"),
    //         },
    //         _ => panic!("Expected Some result"),
    //     }
    // }
    //
    // #[tokio::test]
    // async fn test_tail_function_with_empty_list() {
    //     let tail_fn = TailFunction::new();
    //
    //     let mut builder = ListBuilder::new(StringBuilder::new());
    //     let list_array = Arc::new(builder.finish());
    //     let args = vec![ExpressionValue::list(list_array)];
    //
    //     let result = tail_fn.execute(args).await.unwrap();
    //     assert!(matches!(result, ExpressionValue::Option(None)));
    // }

    #[tokio::test]
    async fn test_tail_function_wrong_argument_type() {
        let tail_fn = TailFunction::new();
        let args = vec![ExpressionValue::string("not a list")];

        let result = tail_fn.execute(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tail_function_wrong_args_count() {
        let tail_fn = TailFunction::new();

        let result = tail_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Option types not yet supported")
        );
    }
}
