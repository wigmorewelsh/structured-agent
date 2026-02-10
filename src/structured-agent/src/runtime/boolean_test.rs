use super::*;
use crate::compiler::CompilationUnit;
use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::sync::Mutex;

use std::sync::Arc;
use tokio;

fn program(source: &str) -> CompilationUnit {
    CompilationUnit::from_string(source.to_string())
}

#[derive(Debug)]
struct BooleanLoggingFunction {
    messages: Arc<Mutex<Vec<String>>>,
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl BooleanLoggingFunction {
    fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            parameters: vec![Parameter::new("value".to_string(), Type::boolean())],
            return_type: Type::unit(),
        }
    }

    fn clear(&self) {
        self.messages.lock().unwrap().clear();
    }
}

#[async_trait(?Send)]
impl NativeFunction for BooleanLoggingFunction {
    fn name(&self) -> &str {
        "log_bool"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        if args.len() != 1 {
            return Err("Expected 1 argument".to_string());
        }

        match &args[0] {
            ExpressionValue::Boolean(b) => {
                self.messages.lock().unwrap().push(b.to_string());
                Ok(ExpressionValue::Unit)
            }
            _ => Err("Expected boolean argument".to_string()),
        }
    }
}

#[derive(Debug)]
struct BooleanReturnFunction {
    return_value: bool,
    parameters: Vec<Parameter>,
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

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, _args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        Ok(ExpressionValue::Boolean(self.return_value))
    }
}

#[tokio::test]
async fn test_boolean_literal_true() {
    let logger = Arc::new(BooleanLoggingFunction::new());

    let program_source = r#"
extern fn log_bool(value: Boolean): ()

fn main(): () {
    let result = log_bool(true)
    result!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_native_function(logger.clone())
        .build();

    let result = runtime.run().await;
    let result = result.unwrap();

    let messages = logger.messages.lock().unwrap().clone();
    assert_eq!(messages, vec!["true"]);

    assert_eq!(result, ExpressionValue::Unit);
}

#[tokio::test]
async fn test_boolean_literal_false() {
    let logger = Arc::new(BooleanLoggingFunction::new());

    let program_source = r#"
extern fn log_bool(value: Boolean): ()

fn main(): () {
    let result = log_bool(false)
    result!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_native_function(logger.clone())
        .build();

    let result = runtime.run().await;
    let result = result.unwrap();

    let messages = logger.messages.lock().unwrap().clone();
    assert_eq!(messages, vec!["false"]);

    assert_eq!(result, ExpressionValue::Unit);
}

#[tokio::test]
async fn test_boolean_variable_assignment() {
    let logger = Arc::new(BooleanLoggingFunction::new());

    let program_source = r#"
extern fn log_bool(value: Boolean): ()

fn main(): () {
    let is_complete = true
    let log_result = log_bool(is_complete)
    let is_ready = false
    let log_result2 = log_bool(is_ready)
    log_result!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_native_function(logger.clone())
        .build();

    let result = runtime.run().await;
    let result = result.unwrap();

    let messages = logger.messages.lock().unwrap().clone();
    assert_eq!(messages.len(), 2);
    assert!(messages.contains(&"true".to_string()));
    assert!(messages.contains(&"false".to_string()));

    assert_eq!(result, ExpressionValue::Unit);
}

#[tokio::test]
async fn test_boolean_function_return() {
    let bool_fn = Arc::new(BooleanReturnFunction::new(true));

    let program_source = r#"
extern fn get_bool(): Boolean

fn check_status(): Boolean {
    let status = get_bool()
    status!
}

fn main(): String {
    let result = check_status()
    "Function completed"!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_native_function(bool_fn.clone())
        .build();

    let result = runtime.run().await;
    let result = result.unwrap();

    assert_eq!(
        result,
        ExpressionValue::String("Function completed".to_string())
    );
}

#[tokio::test]
async fn test_mixed_boolean_and_string_variables() {
    let logger = Arc::new(BooleanLoggingFunction::new());

    let program_source = r#"
extern fn log_bool(value: Boolean): ()

fn main(): String {
    let message = "Processing complete"
    let success = true
    let log_result = log_bool(success)
    message!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_native_function(logger.clone())
        .build();

    let result = runtime.run().await;
    let result = result.unwrap();

    let messages = logger.messages.lock().unwrap().clone();
    assert_eq!(messages, vec!["true"]);

    assert_eq!(
        result,
        ExpressionValue::String("<message>\nProcessing complete\n</message>".to_string())
    );
}
