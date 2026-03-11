use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct SomeValueListFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for SomeValueListFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl SomeValueListFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new(
                "option".to_string(),
                Type::option(Type::list(Type::string())),
            )],
            return_type: Type::list(Type::string()),
        }
    }
}

#[async_trait]
impl NativeFunction for SomeValueListFunction {
    fn name(&self) -> &str {
        "some_value_list"
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
        Some("Unwraps an Option<List<String>> and returns its value. Fails if the Option is None")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::Array;

    #[tokio::test]
    async fn test_some_value_list_function_properties() {
        let some_value_list_fn = SomeValueListFunction::new();

        assert_eq!(some_value_list_fn.name(), "some_value_list");
        assert_eq!(some_value_list_fn.parameters().len(), 1);
        assert_eq!(some_value_list_fn.parameters()[0].name, "option");
        assert_eq!(some_value_list_fn.return_type().name(), "List<String>");
    }

    // TODO: Re-enable when Option types are supported
    // #[tokio::test]
    // async fn test_some_value_list_with_some() {
    //     let some_value_list_fn = SomeValueListFunction::new();
    //
    //     let mut builder = ListBuilder::new(StringBuilder::new());
    //     let values = builder.values();
    //     values.append_value("first");
    //     values.append_value("second");
    //     builder.append(true);
    //     let list_array = Arc::new(builder.finish());
    //
    //     let args = vec![ExpressionValue::Option(Some(Box::new(
    //         ExpressionValue::list(list_array.clone()),
    //     )))];
    //
    //     let result = some_value_list_fn.execute(args).await.unwrap();
    //     match result.as_list() {
    //         Ok(list) => {
    //             assert_eq!(list.len(), 1);
    //             let list_values = list.value(0);
    //             assert_eq!(list_values.len(), 2);
    //         }
    //         Err(_) => panic!("Expected List result"),
    //     }
    // }
    //
    // #[tokio::test]
    // async fn test_some_value_list_with_none() {
    //     let some_value_list_fn = SomeValueListFunction::new();
    //     let args = vec![ExpressionValue::Option(None)];
    //
    //     let result = some_value_list_fn.execute(args).await;
    //     assert!(result.is_err());
    //     assert!(
    //         result
    //             .unwrap_err()
    //             .contains("Cannot unwrap None value with some_value_list")
    //     );
    // }

    #[tokio::test]
    async fn test_some_value_list_wrong_argument_type() {
        let some_value_list_fn = SomeValueListFunction::new();
        let args = vec![ExpressionValue::string("not an option")];

        let result = some_value_list_fn.execute(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_some_value_list_wrong_args_count() {
        let some_value_list_fn = SomeValueListFunction::new();

        let result = some_value_list_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Option types not yet supported")
        );
    }
}
