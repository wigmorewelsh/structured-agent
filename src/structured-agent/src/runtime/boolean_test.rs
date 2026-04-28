use super::*;

use crate::runtime::ExpressionValue;
use std::sync::Arc;
use structured_agent_macros::sa_module;
use tokio;

#[sa_module]
mod bool_fns {
    use std::sync::Mutex;
    use structured_agent_runtime::BooleanValue;

    pub static LOG_BOOL_MESSAGES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    #[sa_fn]
    async fn log_bool(value: BooleanValue) {
        LOG_BOOL_MESSAGES.lock().unwrap().push(value.0.to_string());
    }

    #[sa_fn]
    async fn get_bool() -> BooleanValue {
        BooleanValue(true)
    }
}

#[tokio::test]
async fn test_boolean_literal_true() {
    let program_source = r#"
use bool_fns::log_bool

fn main(): () {
    let result = log_bool(true)
    result!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(bool_fns::BoolFnsModule))
        .build();

    let result = runtime.run().await.unwrap();

    let messages = bool_fns::LOG_BOOL_MESSAGES.lock().unwrap().clone();
    assert_eq!(messages, vec!["true"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_boolean_literal_false() {
    let program_source = r#"
use bool_fns::log_bool

fn main(): () {
    let result = log_bool(false)
    result!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(bool_fns::BoolFnsModule))
        .build();

    let result = runtime.run().await.unwrap();

    let messages = bool_fns::LOG_BOOL_MESSAGES.lock().unwrap().clone();
    assert_eq!(messages, vec!["false"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_boolean_variable_assignment() {
    let program_source = r#"
use bool_fns::log_bool

fn main(): () {
    let is_complete = true
    let log_result = log_bool(is_complete)
    let is_ready = false
    let log_result2 = log_bool(is_ready)
    log_result!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(bool_fns::BoolFnsModule))
        .build();

    let result = runtime.run().await.unwrap();

    let messages = bool_fns::LOG_BOOL_MESSAGES.lock().unwrap().clone();
    assert_eq!(messages.len(), 2);
    assert!(messages.contains(&"true".to_string()));
    assert!(messages.contains(&"false".to_string()));
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_boolean_function_return() {
    let program_source = r#"
use bool_fns::get_bool

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
        .with_module(Arc::new(bool_fns::BoolFnsModule))
        .build();

    let result = runtime.run().await.unwrap();
    assert_eq!(result.as_string().unwrap(), "Function completed");
}

#[tokio::test]
async fn test_mixed_boolean_and_string_variables() {
    let program_source = r#"
use bool_fns::log_bool

fn main(): String {
    let message = "Processing complete"
    let success = true
    let log_result = log_bool(success)
    message!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(bool_fns::BoolFnsModule))
        .build();

    let result = runtime.run().await.unwrap();

    let messages = bool_fns::LOG_BOOL_MESSAGES.lock().unwrap().clone();
    assert_eq!(messages, vec!["true"]);
    assert_eq!(result.as_string().unwrap(), "Processing complete");
}
