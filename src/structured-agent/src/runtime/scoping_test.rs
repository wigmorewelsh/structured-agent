use super::*;
use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Type};
use async_trait::async_trait;
use dashmap::DashSet;

use std::sync::Arc;
use tokio;

#[derive(Debug)]
struct LoggingFunction {
    messages: DashSet<String>,
    parameters: Vec<(String, Type)>,
    return_type: Type,
}

impl LoggingFunction {
    fn new() -> Self {
        Self {
            messages: DashSet::new(),
            parameters: vec![("str".to_string(), Type::string())],
            return_type: Type::unit(),
        }
    }

    fn clear(&self) {
        self.messages.clear();
    }
}

#[async_trait(?Send)]
impl NativeFunction for LoggingFunction {
    fn name(&self) -> &str {
        "log"
    }

    fn parameters(&self) -> &[(String, Type)] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
        if args.len() != 1 {
            return Err("Expected 1 argument".to_string());
        }

        match &args[0] {
            ExprResult::String(s) => {
                self.messages.insert(s.clone());
            }
            _ => return Err("Expected string argument".to_string()),
        }

        Ok(ExprResult::Unit)
    }
}

#[tokio::test]
async fn test_calling_log_should_receive_literals() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> String {
    let result = log("value1")
    result!
}
"#;

    let result = runtime.run(program_source).await;
    let result = result.unwrap();

    let messages = logger
        .messages
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>();
    assert_eq!(messages, vec!["value1"]);

    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_calling_log_should_receive_variables() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
        fn inner_fn() -> String {
            let log_result = log(outer_var)
            let sample = "sample"
            sample!
        }

        fn main() -> String {
            let outer_var = "value1"
            let log_result = log(outer_var)
            let inner_result = inner_fn()
            "some val"!
        }
"#;

    let result = runtime.run(program_source).await;
    let result = result.unwrap();

    let messages = logger
        .messages
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>();
    assert_eq!(messages, vec!["value1"]);

    assert_eq!(result, ExprResult::String("some val".to_string()));
}
