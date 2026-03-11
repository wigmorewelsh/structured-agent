use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use arrow::array::{Array, ListArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{Field, FieldRef};
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

    async fn execute(&self, args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        if args.len() != 1 {
            return Err(format!("tail expects 1 argument, got {}", args.len()));
        }

        let list = args[0]
            .as_list()
            .map_err(|_| "tail expects a list argument")?;

        if list.is_empty() {
            return Ok(ExpressionValue::option_none());
        }

        let values = list.value(0);
        if values.is_empty() {
            return Ok(ExpressionValue::option_none());
        }

        let tail_values = values.slice(1, values.len() - 1);
        let field: FieldRef = Arc::new(Field::new("item", tail_values.data_type().clone(), true));
        let offsets = OffsetBuffer::from_lengths([tail_values.len()]);
        let tail_list = Arc::new(
            ListArray::try_new(field, offsets, tail_values, None)
                .map_err(|e| format!("Failed to create tail list: {}", e))?,
        );

        Ok(ExpressionValue::option_some(ExpressionValue::list(
            tail_list,
        )))
    }

    fn documentation(&self) -> Option<&str> {
        Some("Returns all elements except the first as Option<List>, or None if the list is empty")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Array, ListBuilder, StringArray, StringBuilder};

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
        builder.values().append_value("first");
        builder.values().append_value("second");
        builder.values().append_value("third");
        builder.append(true);

        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];

        let result = tail_fn.execute(args).await.unwrap();
        let inner = result.as_option().unwrap().unwrap();
        let tail_list = inner.as_list().unwrap();
        assert_eq!(tail_list.len(), 1);
        let tail_values = tail_list.value(0);
        let tail_strings = tail_values.as_any().downcast_ref::<StringArray>().unwrap();
        assert_eq!(tail_strings.len(), 2);
        assert_eq!(tail_strings.value(0), "second");
        assert_eq!(tail_strings.value(1), "third");
    }

    #[tokio::test]
    async fn test_tail_function_with_single_element() {
        let tail_fn = TailFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("only");
        builder.append(true);

        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];

        let result = tail_fn.execute(args).await.unwrap();
        let inner = result.as_option().unwrap().unwrap();
        let tail_list = inner.as_list().unwrap();
        assert_eq!(tail_list.len(), 1);
        assert_eq!(tail_list.value(0).len(), 0);
    }

    #[tokio::test]
    async fn test_tail_function_with_empty_list() {
        let tail_fn = TailFunction::new();

        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];

        let result = tail_fn.execute(args).await.unwrap();
        assert!(result.as_option().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_tail_function_wrong_argument_type() {
        let tail_fn = TailFunction::new();
        let args = vec![ExpressionValue::string("not a list")];

        let result = tail_fn.execute(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("tail expects a list argument"));
    }

    #[tokio::test]
    async fn test_tail_function_wrong_args_count() {
        let tail_fn = TailFunction::new();

        let result = tail_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("tail expects 1 argument"));
    }
}
