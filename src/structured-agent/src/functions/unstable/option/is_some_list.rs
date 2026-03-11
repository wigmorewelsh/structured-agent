use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct IsSomeListFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for IsSomeListFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl IsSomeListFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new(
                "option".to_string(),
                Type::option(Type::list(Type::string())),
            )],
            return_type: Type::Boolean,
        }
    }
}

#[async_trait]
impl NativeFunction for IsSomeListFunction {
    fn name(&self) -> &str {
        "is_some_list"
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
        Some("Returns true if the Option<List<String>> contains a value, false if it is None")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_some_list_function_properties() {
        let is_some_list_fn = IsSomeListFunction::new();

        assert_eq!(is_some_list_fn.name(), "is_some_list");
        assert_eq!(is_some_list_fn.parameters().len(), 1);
        assert_eq!(is_some_list_fn.parameters()[0].name, "option");
        assert_eq!(is_some_list_fn.return_type().name(), "Boolean");
    }

    // TODO: Re-enable when Option types are supported
    // #[tokio::test]
    // async fn test_is_some_list_with_some_value() {
    //     let is_some_list_fn = IsSomeListFunction::new();
    //
    //     let mut builder = ListBuilder::new(StringBuilder::new());
    //     let values = builder.values();
    //     values.append_value("test");
    //     builder.append(true);
    //     let list_array = Arc::new(builder.finish());
    //
    //     let args = vec![ExpressionValue::Option(Some(Box::new(
    //         ExpressionValue::list(list_array),
    //     )))];
    //
    //     let result = is_some_list_fn.execute(args).await.unwrap();
    //     assert!(matches!(result, ExpressionValue::boolean(true)));
    // }
    //
    // #[tokio::test]
    // async fn test_is_some_list_with_none() {
    //     let is_some_list_fn = IsSomeListFunction::new();
    //     let args = vec![ExpressionValue::Option(None)];
    //
    //     let result = is_some_list_fn.execute(args).await.unwrap();
    //     assert!(matches!(result, ExpressionValue::boolean(false)));
    // }

    #[tokio::test]
    async fn test_is_some_list_wrong_argument_type() {
        let is_some_list_fn = IsSomeListFunction::new();
        let args = vec![ExpressionValue::string("not an option")];

        let result = is_some_list_fn.execute(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_some_list_wrong_args_count() {
        let is_some_list_fn = IsSomeListFunction::new();

        let result = is_some_list_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Option types not yet supported")
        );
    }
}
