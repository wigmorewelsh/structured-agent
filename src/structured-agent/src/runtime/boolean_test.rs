use super::*;
use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Type};
use async_trait::async_trait;
use dashmap::DashSet;

use std::sync::Arc;
use tokio;

#[derive(Debug)]
struct BooleanLoggingFunction {
    messages: DashSet<String>,
    parameters: Vec<(String, Type)>,
    return_type: Type,
}

impl BooleanLoggingFunction {
    fn new() -> Self {
        Self {
            messages: DashSet::new(),
            parameters: vec![("value".to_string(), Type::boolean())],
            return_type: Type::unit(),
        }
    }

    fn clear(&self) {
        self.messages.clear();
    }
}

#[async_trait(?Send)]
impl NativeFunction for BooleanLoggingFunction {
    fn name(&self) -> &str {
        "log_bool"
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
            ExprResult::Boolean(b) => {
                self.messages.insert(b.to_string());
            }
            _ => return Err("Expected boolean argument".to_string()),
        }

        Ok(ExprResult::Unit)
    }
}

#[derive(Debug)]
struct BooleanReturnFunction {
    return_value: bool,
    parameters: Vec<(String, Type)>,
    return_type: Type,
}

impl BooleanReturnFunction {
    fn new(return_value: bool) -> Self {
        Self {
            return_value,
            parameters: vec![],
            return_type: Type::boolean(),
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for BooleanReturnFunction {
    fn name(&self) -> &str {
        "get_bool"
    }

    fn parameters(&self) -> &[(String, Type)] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, _args: Vec<ExprResult>) -> Result<ExprResult, String> {
        Ok(ExprResult::Boolean(self.return_value))
    }
}

#[tokio::test]
async fn test_boolean_literal_true() {
    let logger = Arc::new(BooleanLoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main(): () {
    let result = log_bool(true)
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
    assert_eq!(messages, vec!["true"]);

    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_boolean_literal_false() {
    let logger = Arc::new(BooleanLoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main(): () {
    let result = log_bool(false)
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
    assert_eq!(messages, vec!["false"]);

    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_boolean_variable_assignment() {
    let logger = Arc::new(BooleanLoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main(): () {
    let is_complete = true
    let log_result = log_bool(is_complete)
    let is_ready = false
    let log_result2 = log_bool(is_ready)
    log_result!
}
"#;

    let result = runtime.run(program_source).await;
    let result = result.unwrap();

    let messages = logger
        .messages
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 2);
    assert!(messages.contains(&"true".to_string()));
    assert!(messages.contains(&"false".to_string()));

    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_boolean_function_return() {
    let bool_fn = Arc::new(BooleanReturnFunction::new(true));

    let mut runtime = Runtime::new();
    runtime.register_native_function(bool_fn.clone());

    let program_source = r#"
fn check_status(): Boolean {
    let status = get_bool()
    status!
}

fn main(): String {
    let result = check_status()
    "Function completed"!
}
"#;

    let result = runtime.run(program_source).await;
    let result = result.unwrap();

    assert_eq!(result, ExprResult::String("Function completed".to_string()));
}

#[tokio::test]
async fn test_mixed_boolean_and_string_variables() {
    let logger = Arc::new(BooleanLoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main(): String {
    let message = "Processing complete"
    let success = true
    let log_result = log_bool(success)
    message!
}
"#;

    let result = runtime.run(program_source).await;
    let result = result.unwrap();

    let messages = logger
        .messages
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>();
    assert_eq!(messages, vec!["true"]);

    assert_eq!(
        result,
        ExprResult::String("Processing complete".to_string())
    );
}
