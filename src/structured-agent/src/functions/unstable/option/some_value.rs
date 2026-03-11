use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct SomeValueFunction {
    name: String,
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl SomeValueFunction {
    pub fn new(inner_type: Type) -> Self {
        let name = format!("some_value_{}", Self::type_suffix(&inner_type));
        let return_type = inner_type.clone();
        Self {
            name,
            parameters: vec![Parameter::new(
                "option".to_string(),
                Type::option(inner_type),
            )],
            return_type,
        }
    }

    pub fn for_string() -> Self {
        Self::new(Type::String)
    }

    pub fn for_list() -> Self {
        Self::new(Type::list(Type::String))
    }

    fn type_suffix(t: &Type) -> &'static str {
        match t {
            Type::List(_) => "list",
            _ => "string",
        }
    }
}

#[async_trait]
impl NativeFunction for SomeValueFunction {
    fn name(&self) -> &str {
        &self.name
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        if args.len() != 1 {
            return Err(format!(
                "{} expects 1 argument, got {}",
                self.name,
                args.len()
            ));
        }
        let opt = args[0]
            .as_option()
            .map_err(|_| format!("{} expects an option argument", self.name))?;
        opt.ok_or_else(|| format!("Cannot unwrap None value with {}", self.name))
    }

    fn documentation(&self) -> Option<&str> {
        Some("Unwraps an Option and returns its value. Fails if the Option is None")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Array, ListBuilder, StringBuilder};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_some_value_string_properties() {
        let f = SomeValueFunction::for_string();
        assert_eq!(f.name(), "some_value_string");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(f.parameters()[0].param_type, Type::option(Type::String));
        assert_eq!(f.return_type().name(), "String");
    }

    #[tokio::test]
    async fn test_some_value_list_properties() {
        let f = SomeValueFunction::for_list();
        assert_eq!(f.name(), "some_value_list");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(
            f.parameters()[0].param_type,
            Type::option(Type::list(Type::String))
        );
        assert_eq!(f.return_type().name(), "List<String>");
    }

    #[tokio::test]
    async fn test_some_value_string_with_some() {
        let f = SomeValueFunction::for_string();
        let result = f
            .execute(vec![ExpressionValue::option_some(ExpressionValue::string(
                "test_value",
            ))])
            .await
            .unwrap();
        assert_eq!(result.as_string().unwrap(), "test_value");
    }

    #[tokio::test]
    async fn test_some_value_string_with_none() {
        let f = SomeValueFunction::for_string();
        let result = f.execute(vec![ExpressionValue::option_none()]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Cannot unwrap None value with")
        );
    }

    #[tokio::test]
    async fn test_some_value_list_with_some() {
        let f = SomeValueFunction::for_list();
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("first");
        builder.values().append_value("second");
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let result = f
            .execute(vec![ExpressionValue::option_some(ExpressionValue::list(
                list_array,
            ))])
            .await
            .unwrap();
        let list = result.as_list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list.value(0).len(), 2);
    }

    #[tokio::test]
    async fn test_some_value_list_with_none() {
        let f = SomeValueFunction::for_list();
        let result = f.execute(vec![ExpressionValue::option_none()]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Cannot unwrap None value with")
        );
    }

    #[tokio::test]
    async fn test_some_value_wrong_argument_type() {
        let f = SomeValueFunction::for_string();
        let result = f
            .execute(vec![ExpressionValue::string("not an option")])
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_some_value_wrong_args_count() {
        let f = SomeValueFunction::for_string();
        let result = f.execute(vec![]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expects 1 argument"));
    }
}
