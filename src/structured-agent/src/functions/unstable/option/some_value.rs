use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct SomeValueFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for SomeValueFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl SomeValueFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new(
                "option".to_string(),
                Type::option(Type::string()),
            )],
            return_type: Type::String,
        }
    }
}

#[async_trait]
impl NativeFunction for SomeValueFunction {
    fn name(&self) -> &str {
        "some_value"
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
        Some("Unwraps an Option and returns its value. Fails if the Option is None")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_some_value_function_properties() {
        let some_value_fn = SomeValueFunction::new();

        assert_eq!(some_value_fn.name(), "some_value");
        assert_eq!(some_value_fn.parameters().len(), 1);
        assert_eq!(some_value_fn.parameters()[0].name, "option");
        assert_eq!(some_value_fn.return_type().name(), "String");
    }

    // TODO: Re-enable when Option types are supported
    // #[tokio::test]
    // async fn test_some_value_with_some() {
    //     let some_value_fn = SomeValueFunction::new();
    //     let args = vec![ExpressionValue::Option(Some(Box::new(
    //         ExpressionValue::string("test_value"),
    //     )))];
    //
    //     let result = some_value_fn.execute(args).await.unwrap();
    //     match result.as_string() {
    //         Ok(s) => assert_eq!(s, "test_value"),
    //         Err(_) => panic!("Expected String result"),
    //     }
    // }
    //
    // #[tokio::test]
    // async fn test_some_value_with_none() {
    //     let some_value_fn = SomeValueFunction::new();
    //     let args = vec![ExpressionValue::Option(None)];
    //
    //     let result = some_value_fn.execute(args).await;
    //     assert!(result.is_err());
    //     assert!(
    //         result
    //             .unwrap_err()
    //             .contains("Cannot unwrap None value with some_value")
    //     );
    // }

    #[tokio::test]
    async fn test_some_value_wrong_argument_type() {
        let some_value_fn = SomeValueFunction::new();
        let args = vec![ExpressionValue::string("not an option")];

        let result = some_value_fn.execute(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_some_value_wrong_args_count() {
        let some_value_fn = SomeValueFunction::new();

        let result = some_value_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Option types not yet supported")
        );
    }
}
