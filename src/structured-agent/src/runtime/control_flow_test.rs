use super::*;
use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::sync::Mutex;

use std::sync::Arc;
use tokio;

#[derive(Debug)]
struct LoggingFunction {
    messages: Arc<Mutex<Vec<String>>>,
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl LoggingFunction {
    fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            parameters: vec![Parameter::new("value".to_string(), Type::string())],
            return_type: Type::unit(),
        }
    }

    fn clear(&self) {
        self.messages.lock().unwrap().clear();
    }

    fn messages_vec(&self) -> Vec<String> {
        self.messages.lock().unwrap().clone()
    }
}

#[async_trait(?Send)]
impl NativeFunction for LoggingFunction {
    fn name(&self) -> &str {
        "log"
    }

    fn parameters(&self) -> &[Parameter] {
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
                self.messages.lock().unwrap().push(s.clone());
                Ok(ExprResult::Unit)
            }
            _ => Err("Expected string argument".to_string()),
        }
    }
}

#[derive(Debug)]
struct BooleanFunction {
    return_value: bool,
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl BooleanFunction {
    fn new(return_value: bool) -> Self {
        Self {
            return_value,
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

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
        if !args.is_empty() {
            return Err("Expected 0 arguments".to_string());
        }

        Ok(ExprResult::Boolean(self.return_value))
    }
}

#[tokio::test]
async fn test_if_statement_true_condition() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    if true {
        log("if body executed")
    }
    log("after if")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();

    assert_eq!(messages, vec!["if body executed", "after if"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_if_statement_false_condition() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
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
extern fn log(message: String): ()

fn main(): () {
    let condition = true
    if condition {
        log("condition was true")
    }
    log("after if")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["condition was true", "after if"]);
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
extern fn log(message: String): ()
extern fn get_bool(): Boolean

fn main(): () {
    if get_bool() {
        log("function returned true")
    }
    log("after if")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["function returned true", "after if"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_while_statement_false_condition() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
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
extern fn log(message: String): ()

fn main(): () {
    let continue_loop = true
    while continue_loop {
        log("loop iteration")
        continue_loop = false
    }
    log("after while")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["loop iteration", "after while"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_nested_if_statements() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    if true {
        log("outer if")
        if true {
            log("inner if")
        }
        log("after inner if")
    }
    log("after outer if")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(
        messages,
        vec!["outer if", "inner if", "after inner if", "after outer if"]
    );
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_if_and_while_combined() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
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
        vec!["starting loop", "in loop", "loop done", "all done"]
    );
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_if_statement_non_boolean_condition_error() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    if "not a boolean" {
        log("this should not execute")
    }
}
"#;

    let result = runtime.run(program_source).await;

    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    println!("Actual error: {}", error_message);
    assert!(error_message.contains("Type error"));
    assert_eq!(logger.messages_vec(), Vec::<String>::new());
}

#[tokio::test]
async fn test_while_statement_non_boolean_condition_error() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    while "not a boolean" {
        log("this should not execute")
    }
}
"#;

    let result = runtime.run(program_source).await;

    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    println!("Actual error: {}", error_message);
    assert!(error_message.contains("Type error"));
    assert_eq!(logger.messages_vec(), Vec::<String>::new());
}

#[tokio::test]
async fn test_if_with_variable_assignment_in_body() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    if true {
        let message = "assigned in if"
        log(message)
    }
    log("after if")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["assigned in if", "after if"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_while_with_variable_assignment_in_body() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    let run_once = true
    while run_once {
        let message = "assigned in while"
        log(message)
        run_once = false
    }
    log("after while")
}
"#;

    let result = runtime.run(program_source).await.unwrap();

    let messages = logger.messages_vec();
    assert_eq!(messages, vec!["assigned in while", "after while"]);
    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_else_branch_type_checking() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

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

    let result = runtime.run(program_source).await;

    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    println!("Actual error: {}", error_message);
    assert!(error_message.contains("Type error"));
    assert_eq!(logger.messages_vec(), Vec::<String>::new());
}
