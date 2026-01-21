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

    fn messages_vec(&self) -> Vec<String> {
        let mut messages: Vec<String> = self.messages.iter().map(|m| m.to_string()).collect();
        messages.sort();
        messages
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

#[derive(Debug)]
struct BooleanFunction {
    value: bool,
    parameters: Vec<(String, Type)>,
    return_type: Type,
}

impl BooleanFunction {
    fn new(value: bool) -> Self {
        Self {
            value,
            parameters: vec![],
            return_type: Type::boolean(),
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for BooleanFunction {
    fn name(&self) -> &str {
        "get_bool"
    }

    fn parameters(&self) -> &[(String, Type)] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
        if !args.is_empty() {
            return Err("Expected 0 arguments".to_string());
        }

        Ok(ExprResult::Boolean(self.value))
    }
}

#[tokio::test]
async fn test_if_statement_true_condition() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    if true {
        log("if body executed")
    }
    log("after if")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();

    assert_eq!(messages, vec!["after if", "if body executed"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_if_statement_false_condition() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    if false {
        log("if body not executed")
    }
    log("after if")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();

    assert_eq!(messages, vec!["after if"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_if_statement_with_variable_condition() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    let condition = true
    if condition {
        log("condition was true")
    }
    log("done")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["condition was true", "done"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_if_statement_with_function_condition() {
    let logger = Arc::new(LoggingFunction::new());
    let bool_func = Arc::new(BooleanFunction::new(true));

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());
    runtime.register_native_function(bool_func.clone());

    let program_source = r#"
fn main() -> () {
    if get_bool() {
        log("function returned true")
    }
    log("finished")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["finished", "function returned true"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_while_statement_false_condition() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    while false {
        log("never executed")
    }
    log("after while")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["after while"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_while_statement_with_counter() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    let continue_loop = true
    while continue_loop {
        log("loop iteration")
        continue_loop = false
    }
    log("loop finished")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["loop finished", "loop iteration"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_nested_if_statements() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    if true {
        log("outer if")
        if true {
            log("inner if")
        }
        log("after inner if")
    }
    log("done")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(
        messages,
        vec!["after inner if", "done", "inner if", "outer if"]
    );
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_if_and_while_combined() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
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
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(
        messages,
        vec!["all done", "in loop", "loop done", "starting loop"]
    );
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_if_statement_non_boolean_condition_error() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    if "not a boolean" {
        log("this should not execute")
    }
}
"#;

    let result = runtime.run(program_source).await;

    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    // Parser correctly rejects string literals as if conditions
    assert!(error_message.contains("Parse error"));
    assert_eq!(logger.messages_vec(), Vec::<String>::new());
}

#[tokio::test]
async fn test_while_statement_non_boolean_condition_error() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    while "not a boolean" {
        log("this should not execute")
    }
}
"#;

    let result = runtime.run(program_source).await;

    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    assert!(error_message.contains("Parse error"));
    assert_eq!(logger.messages_vec(), Vec::<String>::new());
}

#[tokio::test]
async fn test_if_with_variable_assignment_in_body() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    if true {
        let message = "assigned in if"
        log(message)
    }
    log("outside if")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["assigned in if", "outside if"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_while_with_variable_assignment_in_body() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
fn main() -> () {
    let run_once = true
    while run_once {
        let message = "assigned in while"
        log(message)
        run_once = false
    }
    log("outside while")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["assigned in while", "outside while"]);
    assert_eq!(result, ExprResult::Unit);
}
