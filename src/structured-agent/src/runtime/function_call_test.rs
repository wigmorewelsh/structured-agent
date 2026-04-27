use super::*;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use structured_agent_macros::sa_module;
use tokio;

#[sa_module]
mod call_fns {
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[sa_fn]
    async fn to_call() {
        CALL_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

#[tokio::test]
async fn test_function_call_with_assignment() {
    let program_source = r#"use call_fns::to_call

fn assign_result(): () {
    let result = to_call()
}

fn main(): () {
    assign_result()
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(call_fns::CallFnsModule))
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok());
    assert_eq!(call_fns::CALL_COUNT.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn test_function_call_with_prompt_result() {
    let program_source = r#"use call_fns::to_call

fn prompt_result(): () {
    to_call()!
}

fn main(): () {
    prompt_result()
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(call_fns::CallFnsModule))
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok());
    assert_eq!(call_fns::CALL_COUNT.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn test_function_call_ignore_result() {
    let program_source = r#"use call_fns::to_call

fn ignore_result(): () {
    to_call()
}

fn main(): () {
    ignore_result()
}
"#;

    let runtime = Runtime::builder(program(program_source))
        .with_module(Arc::new(call_fns::CallFnsModule))
        .build();

    let result = runtime.run().await;
    assert!(result.is_ok());
    assert_eq!(call_fns::CALL_COUNT.load(Ordering::Relaxed), 1);
}
