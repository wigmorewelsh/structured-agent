use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct PrintFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
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

    async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
        if args.len() != 1 {
            return Err(format!("print expects 1 argument, got {}", args.len()));
        }

        let value = match &args[0] {
            ExprResult::String(s) => s.clone(),
            ExprResult::Boolean(b) => b.to_string(),
            ExprResult::Unit => "()".to_string(),
        };

        println!("{}", value);
        Ok(ExprResult::Unit)
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
        let args = vec![ExprResult::String("Hello, World!".to_string())];

        let result = print_fn.execute(args).await.unwrap();
        assert_eq!(result, ExprResult::Unit);
    }

    #[tokio::test]
    async fn test_print_function_execute_boolean() {
        let print_fn = PrintFunction::new();
        let args = vec![ExprResult::Boolean(true)];

        let result = print_fn.execute(args).await.unwrap();
        assert_eq!(result, ExprResult::Unit);
    }

    #[tokio::test]
    async fn test_print_function_execute_unit() {
        let print_fn = PrintFunction::new();
        let args = vec![ExprResult::Unit];

        let result = print_fn.execute(args).await.unwrap();
        assert_eq!(result, ExprResult::Unit);
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
                ExprResult::String("a".to_string()),
                ExprResult::String("b".to_string()),
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
