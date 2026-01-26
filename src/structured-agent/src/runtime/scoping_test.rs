use super::*;
use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
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
            parameters: vec![Parameter::new("str".to_string(), Type::string())],
            return_type: Type::unit(),
        }
    }

    fn clear(&self) {
        self.messages.lock().unwrap().clear();
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
extern fn log(message: String): ()

fn main(): String {
    let result = log("value1")
    result!
}
"#;

    let result = runtime.run(program_source).await;
    let result = result.unwrap();

    let messages = logger.messages.lock().unwrap().clone();
    assert_eq!(messages, vec!["value1"]);

    assert_eq!(result, ExprResult::Unit);
}

#[tokio::test]
async fn test_variable_assignment_in_if_block() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

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

    let result = runtime.run(program_source).await;
    let result = result.unwrap();

    let messages = logger.messages.lock().unwrap().clone();

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

    assert_eq!(result, ExprResult::String("modified".to_string()));
}

#[tokio::test]
async fn test_variable_assignment_in_while_loop() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

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

    let result = runtime.run(program_source).await;
    let result = result.unwrap();

    let messages = logger.messages.lock().unwrap().clone();

    // Check that the variable was modified inside the while loop
    assert!(messages.contains(&"0".to_string())); // Initial value in loop
    assert!(messages.contains(&"1".to_string())); // Modified value after loop

    // The function should return the modified value from inside the loop
    assert_eq!(result, ExprResult::String("1".to_string()));
}

#[tokio::test]
async fn test_variable_scoping_with_boolean_assignment() {
    let logger = Arc::new(LoggingFunction::new());

    let mut runtime = Runtime::new();
    runtime.register_native_function(logger.clone());

    let program_source = r#"
extern fn log(message: String): ()

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

    let result = runtime.run(program_source).await;
    assert!(result.is_ok());

    let messages = logger.messages.lock().unwrap().clone();

    // Should see the initial true check and the assignment working
    assert!(messages.contains(&"before assignment".to_string()));
    assert!(messages.contains(&"after assignment".to_string()));

    // Should NOT see the error message if scoping worked
    assert!(!messages.contains(&"ERROR still true".to_string()));

    // Should not see the false branch
    assert!(!messages.contains(&"this should not print".to_string()));
}

#[tokio::test]
async fn test_context_assign_variable_directly() {
    let runtime = Rc::new(Runtime::new());
    let context = Arc::new(Context::with_runtime(runtime));

    context.declare_variable(
        "test_var".to_string(),
        ExprResult::String("initial".to_string()),
    );

    assert_eq!(
        context.get_variable("test_var").unwrap(),
        ExprResult::String("initial".to_string())
    );

    let result = context.assign_variable(
        "test_var".to_string(),
        ExprResult::String("modified".to_string()),
    );
    assert!(result.is_ok(), "assign_variable should succeed");

    assert_eq!(
        context.get_variable("test_var").unwrap(),
        ExprResult::String("modified".to_string())
    );
}

#[tokio::test]
async fn test_variable_assignment_expr_directly() {
    use crate::expressions::{StringLiteralExpr, VariableAssignmentExpr};
    use crate::types::Expression;

    let runtime = Rc::new(Runtime::new());
    let context = Arc::new(Context::with_runtime(runtime));

    context.declare_variable(
        "test_var".to_string(),
        ExprResult::String("initial".to_string()),
    );

    let assignment = VariableAssignmentExpr {
        variable: "test_var".to_string(),
        expression: Box::new(StringLiteralExpr {
            value: "modified".to_string(),
        }),
    };

    let result = assignment.evaluate(context.clone()).await;
    assert!(
        result.is_ok(),
        "VariableAssignmentExpr evaluation should succeed"
    );

    assert_eq!(
        context.get_variable("test_var").unwrap(),
        ExprResult::String("modified".to_string())
    );
}
