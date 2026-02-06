use super::*;
use crate::compiler::CompilationUnit;
use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

use std::sync::Arc;
use tokio;

fn program(source: &str) -> CompilationUnit {
    CompilationUnit::from_string(source.to_string())
}

#[derive(Debug)]
struct TestExternFunction {
    call_count: std::sync::atomic::AtomicUsize,
    return_type: Type,
}

impl TestExternFunction {
    fn new() -> Self {
        Self {
            call_count: std::sync::atomic::AtomicUsize::new(0),
            return_type: Type::unit(),
        }
    }

    fn get_call_count(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[async_trait(?Send)]
impl NativeFunction for TestExternFunction {
    fn name(&self) -> &str {
        "to_call"
    }

    fn parameters(&self) -> &[Parameter] {
        &[]
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
        if !args.is_empty() {
            return Err("Expected no arguments".to_string());
        }

        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(ExprResult::Unit)
    }
}

#[tokio::test]
async fn test_function_call_with_assignment() {
    let extern_fn = Arc::new(TestExternFunction::new());

    let program_source = r#"
extern fn to_call(): ()

fn assign_result(): () {
    let result = to_call()
}

fn main(): () {
    assign_result()
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_native_function(extern_fn.clone())
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok());
    assert_eq!(extern_fn.get_call_count(), 1);
}

#[tokio::test]
async fn test_function_call_with_prompt_result() {
    let extern_fn = Arc::new(TestExternFunction::new());

    let program_source = r#"
extern fn to_call(): ()

fn prompt_result(): () {
    to_call()!
}

fn main(): () {
    prompt_result()
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_native_function(extern_fn.clone())
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok());
    assert_eq!(extern_fn.get_call_count(), 1);
}

#[tokio::test]
async fn test_function_call_ignore_result() {
    let extern_fn = Arc::new(TestExternFunction::new());

    let program_source = r#"
extern fn to_call(): ()

fn ignore_result(): () {
    to_call()
}

fn main(): () {
    ignore_result()
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_native_function(extern_fn.clone())
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok());
    assert_eq!(extern_fn.get_call_count(), 1);
}
