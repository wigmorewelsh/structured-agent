use crate::runtime::{AgentHandle, ExpressionValue};
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct IsSomeFunction {
    name: String,
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl IsSomeFunction {
    pub fn new(inner_type: Type) -> Self {
        let name = format!("is_some_{}", Self::type_suffix(&inner_type));
        Self {
            name,
            parameters: vec![Parameter::new(
                "option".to_string(),
                Type::option(inner_type),
            )],
            return_type: Type::Boolean,
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
impl NativeFunction for IsSomeFunction {
    fn name(&self) -> &str {
        &self.name
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(
        &self,
        args: Vec<ExpressionValue>,
        _agent: &AgentHandle,
    ) -> Result<ExpressionValue, String> {
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
        Ok(ExpressionValue::boolean(opt.is_some()))
    }

    fn documentation(&self) -> Option<&str> {
        Some("Returns true if the Option contains a value, false if it is None")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{ListBuilder, StringBuilder};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_is_some_string_properties() {
        let f = IsSomeFunction::for_string();
        assert_eq!(f.name(), "is_some_string");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(f.parameters()[0].param_type, Type::option(Type::String));
        assert_eq!(f.return_type().name(), "Boolean");
    }

    #[tokio::test]
    async fn test_is_some_list_properties() {
        let f = IsSomeFunction::for_list();
        assert_eq!(f.name(), "is_some_list");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(
            f.parameters()[0].param_type,
            Type::option(Type::list(Type::String))
        );
        assert_eq!(f.return_type().name(), "Boolean");
    }

    #[tokio::test]
    async fn test_is_some_string_with_some() {
        let f = IsSomeFunction::for_string();
        let result = f
            .execute(
                vec![ExpressionValue::option_some(ExpressionValue::string(
                    "value",
                ))],
                &AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_boolean().unwrap(), true);
    }

    #[tokio::test]
    async fn test_is_some_string_with_none() {
        let f = IsSomeFunction::for_string();
        let result = f
            .execute(
                vec![ExpressionValue::option_none()],
                &AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_boolean().unwrap(), false);
    }

    #[tokio::test]
    async fn test_is_some_list_with_some() {
        let f = IsSomeFunction::for_list();
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("test");
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let result = f
            .execute(
                vec![ExpressionValue::option_some(ExpressionValue::list(
                    list_array,
                ))],
                &AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_boolean().unwrap(), true);
    }

    #[tokio::test]
    async fn test_is_some_list_with_none() {
        let f = IsSomeFunction::for_list();
        let result = f
            .execute(
                vec![ExpressionValue::option_none()],
                &AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_boolean().unwrap(), false);
    }

    #[tokio::test]
    async fn test_is_some_wrong_argument_type() {
        let f = IsSomeFunction::for_string();
        let result = f
            .execute(
                vec![ExpressionValue::string("not an option")],
                &AgentHandle::detached(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_some_wrong_args_count() {
        let f = IsSomeFunction::for_string();
        let result = f.execute(vec![], &AgentHandle::detached()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expects 1 argument"));
    }
}
