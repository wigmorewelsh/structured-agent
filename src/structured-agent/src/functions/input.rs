use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Type};
use async_trait::async_trait;
use std::io::{self, Write};

#[derive(Debug)]
pub struct InputFunction {
    return_type: Type,
}

impl InputFunction {
    pub fn new() -> Self {
        Self {
            return_type: Type::string(),
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for InputFunction {
    fn name(&self) -> &str {
        "input"
    }

    fn parameters(&self) -> &[(String, Type)] {
        &[]
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, _args: Vec<ExprResult>) -> Result<ExprResult, String> {
        print!("> ");
        io::stdout()
            .flush()
            .map_err(|e| format!("Failed to flush stdout: {}", e))?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read input: {}", e))?;

        let trimmed = input.trim().to_string();
        Ok(ExprResult::String(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_input_function_properties() {
        let input_fn = InputFunction::new();

        assert_eq!(input_fn.name(), "input");
        assert_eq!(input_fn.parameters().len(), 0);
        assert_eq!(input_fn.return_type().name, "String");
    }

    #[tokio::test]
    async fn test_input_function_debug() {
        let input_fn = InputFunction::new();
        let debug_output = format!("{:?}", input_fn);
        assert!(debug_output.contains("InputFunction"));
    }
}
