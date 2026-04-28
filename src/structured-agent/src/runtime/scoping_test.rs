use super::*;
use crate::runtime::ExpressionValue;
use std::sync::Arc;
use structured_agent_macros::sa_module;
use tokio;

#[sa_module]
mod log_fns {
    use std::sync::Mutex;
    use structured_agent_runtime::StringValue;

    pub static MESSAGES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    #[sa_fn]
    async fn log(message: StringValue) {
        MESSAGES.lock().unwrap().push(message.0);
    }
}

#[tokio::test]
async fn test_calling_log_should_receive_literals() {
    let program_source = r#"
use log_fns::log

fn main(): () {
    let result = log("value1")
    result!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(log_fns::LogFnsModule))
        .build();

    let result = runtime.run().await.unwrap();
    let messages = log_fns::MESSAGES.lock().unwrap().clone();
    assert_eq!(messages, vec!["value1"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_variable_assignment_in_if_block() {
    let program_source = r#"
use log_fns::log

fn main(): String {
    let val = "initial"
    log("step1")
    log(val)
    if true {
        log("step2")
        val = "modified"
        log(val)
        log("step3")
    }
    log("step4")
    log(val)
    val!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(log_fns::LogFnsModule))
        .build();

    let result = runtime.run().await.unwrap();
    let messages = log_fns::MESSAGES.lock().unwrap().clone();

    assert!(messages.contains(&"step1".to_string()));
    assert!(messages.contains(&"step2".to_string()));
    assert!(messages.contains(&"step3".to_string()));
    assert!(messages.contains(&"step4".to_string()));
    assert!(messages.contains(&"initial".to_string()));
    assert!(messages.contains(&"modified".to_string()));

    let modified_count = messages.iter().filter(|&m| m == "modified").count();
    assert_eq!(
        modified_count, 2,
        "Should see 'modified' twice if variable assignment persists"
    );

    assert_eq!(result, ExpressionValue::string("modified"));
}

#[tokio::test]
async fn test_variable_assignment_in_while_loop() {
    let program_source = r#"
use log_fns::log

fn main(): String {
    let counter = true
    let iteration = "0"

    while counter {
        log(iteration)
        iteration = "1"
        counter = false
    }

    log(iteration)
    iteration!
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(log_fns::LogFnsModule))
        .build();

    let result = runtime.run().await.unwrap();
    let messages = log_fns::MESSAGES.lock().unwrap().clone();

    assert!(messages.contains(&"0".to_string()));
    assert!(messages.contains(&"1".to_string()));
    assert_eq!(result, ExpressionValue::string("1"));
}

#[tokio::test]
async fn test_variable_scoping_with_boolean_assignment() {
    let program_source = r#"
use log_fns::log

fn main(): () {
    let val = true

    if val {
        log("before assignment")
        val = false
        log("after assignment")
    }

    if val {
        log("ERROR still true")
    }

    if false {
        log("this should not print")
    }
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(log_fns::LogFnsModule))
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok());

    let messages = log_fns::MESSAGES.lock().unwrap().clone();

    assert!(messages.contains(&"before assignment".to_string()));
    assert!(messages.contains(&"after assignment".to_string()));
    assert!(!messages.contains(&"ERROR still true".to_string()));
    assert!(!messages.contains(&"this should not print".to_string()));
}
