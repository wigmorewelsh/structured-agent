use structured_agent_il::Instruction;
use structured_agent_macros::sa_fn;
use structured_agent_runtime::{
    AgentHandle, BooleanValue, ExpressionValue, IntValue, StringValue, Type,
};

#[sa_fn]
async fn greet(name: StringValue) -> StringValue {
    StringValue(format!("Hello, {}!", name.0))
}

#[sa_fn]
async fn no_args() -> StringValue {
    StringValue("hello".to_string())
}

#[sa_fn]
async fn unit_return(value: StringValue) -> () {
    let _ = value;
}

#[sa_fn]
async fn multi_param(a: StringValue, b: IntValue, c: BooleanValue) -> StringValue {
    StringValue(format!("{} {} {}", a.0, b.0, c.0))
}

/// Documented function
#[sa_fn]
async fn documented() -> StringValue {
    StringValue("result".to_string())
}

#[sa_fn]
async fn option_param(value: Option<StringValue>) -> StringValue {
    value
        .map(|v| StringValue(v.0))
        .unwrap_or_else(|| StringValue("none".to_string()))
}

#[sa_fn]
async fn option_return(flag: BooleanValue) -> Option<StringValue> {
    if flag.0 {
        Some(StringValue("yes".to_string()))
    } else {
        None
    }
}

fn call_native_fn(
    def: &structured_agent_il::NativeFunctionDef,
) -> structured_agent_runtime::NativeFnPtr {
    if let Instruction::CallNative { f, .. } = &def.body[0] {
        f.clone()
    } else {
        panic!("expected CallNative instruction");
    }
}

#[tokio::test]
async fn test_greet_name() {
    assert_eq!(greet_native_def().name, "greet");
}

#[tokio::test]
async fn test_greet_parameters() {
    let def = greet_native_def();
    assert_eq!(def.parameters.len(), 1);
    assert_eq!(def.parameters[0].name, "name");
    assert_eq!(def.parameters[0].param_type, Type::string());
}

#[tokio::test]
async fn test_greet_return_type() {
    assert_eq!(greet_native_def().return_type, Type::string());
}

#[tokio::test]
async fn test_greet_execute() {
    let def = greet_native_def();
    let f = call_native_fn(&def);
    let result = f
        .call(
            vec![ExpressionValue::string("World")],
            AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "Hello, World!");
}

#[tokio::test]
async fn test_greet_wrong_arg_count() {
    let def = greet_native_def();
    let f = call_native_fn(&def);
    let err = f.call(vec![], AgentHandle::detached()).await.unwrap_err();
    assert!(
        err.contains("greet expects 1 argument(s), got 0"),
        "{}",
        err
    );
}

#[tokio::test]
async fn test_no_args_parameters() {
    assert_eq!(no_args_native_def().parameters.len(), 0);
}

#[tokio::test]
async fn test_no_args_execute() {
    let def = no_args_native_def();
    let f = call_native_fn(&def);
    let result = f.call(vec![], AgentHandle::detached()).await.unwrap();
    assert_eq!(result.as_string().unwrap(), "hello");
}

#[tokio::test]
async fn test_unit_return_type() {
    assert_eq!(unit_return_native_def().return_type, Type::unit());
}

#[tokio::test]
async fn test_unit_return_execute() {
    let def = unit_return_native_def();
    let f = call_native_fn(&def);
    let result = f
        .call(vec![ExpressionValue::string("x")], AgentHandle::detached())
        .await
        .unwrap();
    assert_eq!(result, ExpressionValue::unit());
}

#[tokio::test]
async fn test_multi_param_execute() {
    let def = multi_param_native_def();
    let f = call_native_fn(&def);
    let result = f
        .call(
            vec![
                ExpressionValue::string("hello"),
                ExpressionValue::integer(42),
                ExpressionValue::boolean(true),
            ],
            AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "hello 42 true");
}

#[tokio::test]
async fn test_documented_function() {
    let def = documented_native_def();
    assert!(def.documentation.is_some());
    assert!(
        def.documentation
            .as_ref()
            .unwrap()
            .contains("Documented function")
    );
}

#[tokio::test]
async fn test_option_param_some() {
    let def = option_param_native_def();
    let f = call_native_fn(&def);
    let result = f
        .call(
            vec![ExpressionValue::option_some(ExpressionValue::string("hi"))],
            AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "hi");
}

#[tokio::test]
async fn test_option_param_none() {
    let def = option_param_native_def();
    let f = call_native_fn(&def);
    let result = f
        .call(
            vec![ExpressionValue::option_none()],
            AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_string().unwrap(), "none");
}

#[tokio::test]
async fn test_option_return_some() {
    let def = option_return_native_def();
    let f = call_native_fn(&def);
    let result = f
        .call(
            vec![ExpressionValue::boolean(true)],
            AgentHandle::detached(),
        )
        .await
        .unwrap();
    let inner = result.as_option().unwrap().unwrap();
    assert_eq!(inner.as_string().unwrap(), "yes");
}

#[tokio::test]
async fn test_option_return_none() {
    let def = option_return_native_def();
    let f = call_native_fn(&def);
    let result = f
        .call(
            vec![ExpressionValue::boolean(false)],
            AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert!(result.as_option().unwrap().is_none());
}
