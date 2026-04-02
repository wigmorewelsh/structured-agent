use structured_agent_macros::sa_fn;
use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFunction as _, Type};

#[sa_fn]
async fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

#[sa_fn]
async fn no_args() -> String {
    "hello".to_string()
}

#[sa_fn]
async fn unit_return(value: String) -> () {
    let _ = value;
}

#[sa_fn]
async fn multi_param(a: String, b: i64, c: bool) -> String {
    format!("{} {} {}", a, b, c)
}

/// Documented function
#[sa_fn]
async fn documented() -> String {
    "result".to_string()
}

#[sa_fn]
async fn option_param(value: Option<String>) -> String {
    value.unwrap_or_else(|| "none".to_string())
}

#[sa_fn]
async fn option_return(flag: bool) -> Option<String> {
    if flag { Some("yes".to_string()) } else { None }
}

#[tokio::test]
async fn test_greet_name() {
    assert_eq!(GreetFunction::new().name(), "greet");
}

#[tokio::test]
async fn test_greet_parameters() {
    let f = GreetFunction::new();
    assert_eq!(f.parameters().len(), 1);
    assert_eq!(f.parameters()[0].name, "name");
    assert_eq!(f.parameters()[0].param_type, Type::string());
}

#[tokio::test]
async fn test_greet_return_type() {
    assert_eq!(GreetFunction::new().return_type(), &Type::string());
}

#[tokio::test]
async fn test_greet_execute() {
    let result = GreetFunction::new()
        .execute(
            vec![ExpressionValue::string("World")],
            &AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "Hello, World!");
}

#[tokio::test]
async fn test_greet_wrong_arg_count() {
    let err = GreetFunction::new()
        .execute(vec![], &AgentHandle::detached())
        .await
        .unwrap_err();
    assert!(
        err.contains("greet expects 1 argument(s), got 0"),
        "{}",
        err
    );
}

#[tokio::test]
async fn test_no_args_parameters() {
    assert_eq!(NoArgsFunction::new().parameters().len(), 0);
}

#[tokio::test]
async fn test_no_args_execute() {
    let result = NoArgsFunction::new()
        .execute(vec![], &AgentHandle::detached())
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "hello");
}

#[tokio::test]
async fn test_unit_return_type() {
    assert_eq!(UnitReturnFunction::new().return_type(), &Type::unit());
}

#[tokio::test]
async fn test_unit_return_execute() {
    let result = UnitReturnFunction::new()
        .execute(vec![ExpressionValue::string("x")], &AgentHandle::detached())
        .await
        .unwrap();
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_multi_param_execute() {
    let result = MultiParamFunction::new()
        .execute(
            vec![
                ExpressionValue::string("hello"),
                ExpressionValue::integer(42),
                ExpressionValue::boolean(true),
            ],
            &AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "hello 42 true");
}

#[tokio::test]
async fn test_documented_function() {
    assert!(DocumentedFunction::new().documentation().is_some());
    assert!(
        DocumentedFunction::new()
            .documentation()
            .unwrap()
            .contains("Documented function")
    );
}

#[tokio::test]
async fn test_option_param_some() {
    let result = OptionParamFunction::new()
        .execute(
            vec![ExpressionValue::option_some(ExpressionValue::string("hi"))],
            &AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "hi");
}

#[tokio::test]
async fn test_option_param_none() {
    let result = OptionParamFunction::new()
        .execute(
            vec![ExpressionValue::option_none()],
            &AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "none");
}

#[tokio::test]
async fn test_option_return_some() {
    let result = OptionReturnFunction::new()
        .execute(
            vec![ExpressionValue::boolean(true)],
            &AgentHandle::detached(),
        )
        .await
        .unwrap();
    let inner = result.as_option().unwrap().unwrap();
    assert_eq!(inner.as_string().unwrap(), "yes");
}

#[tokio::test]
async fn test_option_return_none() {
    let result = OptionReturnFunction::new()
        .execute(
            vec![ExpressionValue::boolean(false)],
            &AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert!(result.as_option().unwrap().is_none());
}
