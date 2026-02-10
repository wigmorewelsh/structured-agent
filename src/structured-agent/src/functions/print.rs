use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use arrow::array::Array;
use async_trait::async_trait;

#[derive(Debug)]
pub struct PrintFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for PrintFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl PrintFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new("value".to_string(), Type::string())],
            return_type: Type::unit(),
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for PrintFunction {
    fn name(&self) -> &str {
        "print"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        if args.len() != 1 {
            return Err(format!("print expects 1 argument, got {}", args.len()));
        }

        fn format_expr_result(result: &ExpressionValue) -> String {
            match result {
                ExpressionValue::String(s) => s.clone(),
                ExpressionValue::Boolean(b) => b.to_string(),
                ExpressionValue::Unit => "()".to_string(),
                ExpressionValue::List(list) => {
                    if list.len() == 0 {
                        "[]".to_string()
                    } else {
                        let values = list.value(0);
                        if let Some(string_array) =
                            values.as_any().downcast_ref::<arrow::array::StringArray>()
                        {
                            let items: Vec<String> = (0..string_array.len())
                                .map(|i| format!("\"{}\"", string_array.value(i)))
                                .collect();
                            format!("[{}]", items.join(", "))
                        } else {
                            "[]".to_string()
                        }
                    }
                }
                ExpressionValue::Option(opt) => match opt {
                    Some(inner) => format!("Some({})", format_expr_result(inner)),
                    None => "None".to_string(),
                },
            }
        }

        let value = format_expr_result(&args[0]);
        println!("{}", value);
        Ok(ExpressionValue::Unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_print_function_properties() {
        let print_fn = PrintFunction::new();

        assert_eq!(print_fn.name(), "print");
        assert_eq!(print_fn.parameters().len(), 1);
        assert_eq!(print_fn.parameters()[0].name, "value");
        assert_eq!(print_fn.parameters()[0].param_type.name(), "String");
        assert_eq!(print_fn.return_type().name(), "()");
    }

    #[tokio::test]
    async fn test_print_function_execute_string() {
        let print_fn = PrintFunction::new();
        let args = vec![ExpressionValue::String("Hello, World!".to_string())];

        let result = print_fn.execute(args).await.unwrap();
        assert_eq!(result, ExpressionValue::Unit);
    }

    #[tokio::test]
    async fn test_print_function_execute_boolean() {
        let print_fn = PrintFunction::new();
        let args = vec![ExpressionValue::Boolean(true)];

        let result = print_fn.execute(args).await.unwrap();
        assert_eq!(result, ExpressionValue::Unit);
    }

    #[tokio::test]
    async fn test_print_function_execute_unit() {
        let print_fn = PrintFunction::new();
        let args = vec![ExpressionValue::Unit];

        let result = print_fn.execute(args).await.unwrap();
        assert_eq!(result, ExpressionValue::Unit);
    }

    #[tokio::test]
    async fn test_print_function_wrong_args_count() {
        let print_fn = PrintFunction::new();

        let result = print_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("print expects 1 argument, got 0")
        );

        let result = print_fn
            .execute(vec![
                ExpressionValue::String("a".to_string()),
                ExpressionValue::String("b".to_string()),
            ])
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("print expects 1 argument, got 2")
        );
    }
}
