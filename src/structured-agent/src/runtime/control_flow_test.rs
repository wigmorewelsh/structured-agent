use super::*;
use crate::runtime::ExpressionValue;
use std::sync::Arc;
use structured_agent_macros::sa_module;
use tokio;

#[sa_module]
mod control_fns {
    use std::sync::Mutex;

    pub static MESSAGES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    #[sa_fn]
    async fn log(message: String) {
        MESSAGES.lock().unwrap().push(message);
    }

    #[sa_fn]
    async fn get_bool() -> bool {
        true
    }
}

const USE_IMPORTS: &str = "use control_fns::log\nuse control_fns::get_bool\n\n";

async fn run_logged(source: &str) -> (ExpressionValue, Vec<String>) {
    let prefixed = format!("{}{}", USE_IMPORTS, source);
    let runtime = Runtime::builder(program(&prefixed))
        .with_module(Arc::new(control_fns::ControlFnsModule))
        .build();
    let result = runtime.run().await.unwrap();
    let messages = control_fns::MESSAGES.lock().unwrap().clone();
    (result, messages)
}

#[tokio::test]
async fn test_if_statement_true_condition() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    if true {
        log("if body executed")
    }
    log("after if")
}"#,
    )
    .await;

    assert_eq!(messages, vec!["if body executed", "after if"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_if_statement_false_condition() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    if false {
        log("if body not executed")
    }
    log("after if")
}"#,
    )
    .await;

    assert_eq!(messages, vec!["after if"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_if_statement_with_variable_condition() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    let condition = true
    if condition {
        log("condition was true")
    }
    log("after if")
}"#,
    )
    .await;

    assert_eq!(messages, vec!["condition was true", "after if"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_if_statement_with_function_condition() {
    let program_source = r#"use control_fns::log
use control_fns::get_bool

fn main(): () {
    if get_bool() {
        log("function returned true")
    }
    log("after if")
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(control_fns::ControlFnsModule))
        .build();

    let result = runtime.run().await.unwrap();

    let messages = control_fns::MESSAGES.lock().unwrap().clone();
    assert_eq!(messages, vec!["function returned true", "after if"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_while_statement_false_condition() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    while false {
        log("never executed")
    }
    log("after while")
}"#,
    )
    .await;

    assert_eq!(messages, vec!["after while"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_while_statement_with_counter() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    let continue_loop = true
    while continue_loop {
        log("loop iteration")
        continue_loop = false
    }
    log("after while")
}"#,
    )
    .await;

    assert_eq!(messages, vec!["loop iteration", "after while"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_nested_if_statements() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    if true {
        log("outer if")
        if true {
            log("inner if")
        }
        log("after inner if")
    }
    log("after outer if")
}"#,
    )
    .await;

    assert_eq!(
        messages,
        vec!["outer if", "inner if", "after inner if", "after outer if"]
    );
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_if_and_while_combined() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    let should_run = true
    if should_run {
        log("starting loop")
        let counter = true
        while counter {
            log("in loop")
            counter = false
        }
        log("loop done")
    }
    log("all done")
}"#,
    )
    .await;

    assert_eq!(
        messages,
        vec!["starting loop", "in loop", "loop done", "all done"]
    );
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_if_statement_non_boolean_condition_error() {
    let program_source = r#"use control_fns::log

fn main(): () {
    if "not a boolean" {
        log("this should not execute")
    }
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(control_fns::ControlFnsModule))
        .build();

    let result = runtime.run().await;

    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    assert!(error_message.contains("Type error"));
    assert_eq!(
        control_fns::MESSAGES.lock().unwrap().clone(),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn test_while_statement_non_boolean_condition_error() {
    let program_source = r#"use control_fns::log

fn main(): () {
    while "not a boolean" {
        log("this should not execute")
    }
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(control_fns::ControlFnsModule))
        .build();

    let result = runtime.run().await;

    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    assert!(error_message.contains("Type error"));
    assert_eq!(
        control_fns::MESSAGES.lock().unwrap().clone(),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn test_if_with_variable_assignment_in_body() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    if true {
        let message = "assigned in if"
        log(message)
    }
    log("after if")
}"#,
    )
    .await;

    assert_eq!(messages, vec!["assigned in if", "after if"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_while_with_variable_assignment_in_body() {
    let (result, messages) = run_logged(
        r#"fn main(): () {
    let run_once = true
    while run_once {
        let message = "assigned in while"
        log(message)
        run_once = false
    }
    log("after while")
}"#,
    )
    .await;

    assert_eq!(messages, vec!["assigned in while", "after while"]);
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_else_branch_type_checking() {
    let program_source = r#"use control_fns::log

fn main(): () {
    if true {
        log("if branch ok")
    } else {
        if "not a boolean" {
            log("bad")
        }
    }
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(control_fns::ControlFnsModule))
        .build();

    let result = runtime.run().await;

    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    assert!(error_message.contains("Type error"));
    assert_eq!(
        control_fns::MESSAGES.lock().unwrap().clone(),
        Vec::<String>::new()
    );
}
