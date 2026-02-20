use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct IsSomeFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for IsSomeFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl IsSomeFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new(
                "option".to_string(),
                Type::option(Type::string()),
            )],
            return_type: Type::Boolean,
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for IsSomeFunction {
    fn name(&self) -> &str {
        "is_some"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        if args.len() != 1 {
            return Err(format!("is_some expects 1 argument, got {}", args.len()));
        }

        match &args[0] {
            ExpressionValue::Option(opt) => Ok(ExpressionValue::Boolean(opt.is_some())),
            _ => Err("is_some expects an Option argument".to_string()),
        }
    }

    fn documentation(&self) -> Option<&str> {
        Some("Returns true if the Option contains a value, false if it is None")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_some_function_properties() {
        let is_some_fn = IsSomeFunction::new();

        assert_eq!(is_some_fn.name(), "is_some");
        assert_eq!(is_some_fn.parameters().len(), 1);
        assert_eq!(is_some_fn.parameters()[0].name, "option");
        assert_eq!(is_some_fn.return_type().name(), "Boolean");
    }

    #[tokio::test]
    async fn test_is_some_with_some_value() {
        let is_some_fn = IsSomeFunction::new();
        let args = vec![ExpressionValue::Option(Some(Box::new(
            ExpressionValue::String("value".to_string()),
        )))];

        let result = is_some_fn.execute(args).await.unwrap();
        assert!(matches!(result, ExpressionValue::Boolean(true)));
    }

    #[tokio::test]
    async fn test_is_some_with_none() {
        let is_some_fn = IsSomeFunction::new();
        let args = vec![ExpressionValue::Option(None)];

        let result = is_some_fn.execute(args).await.unwrap();
        assert!(matches!(result, ExpressionValue::Boolean(false)));
    }

    #[tokio::test]
    async fn test_is_some_wrong_argument_type() {
        let is_some_fn = IsSomeFunction::new();
        let args = vec![ExpressionValue::String("not an option".to_string())];

        let result = is_some_fn.execute(args).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("is_some expects an Option argument")
        );
    }

    #[tokio::test]
    async fn test_is_some_wrong_args_count() {
        let is_some_fn = IsSomeFunction::new();

        let result = is_some_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("is_some expects 1 argument, got 0")
        );
    }
}
