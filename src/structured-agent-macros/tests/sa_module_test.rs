use structured_agent_macros::sa_module;
use structured_agent_runtime::{AgentHandle, ExpressionValue, Module};

#[sa_module]
mod math {
    /// Add two integers
    #[sa_fn]
    async fn add(a: i64, b: i64) -> i64 {
        a + b
    }

    /// Negate a boolean
    #[sa_fn]
    async fn negate(value: bool) -> bool {
        !value
    }
}

#[test]
fn test_module_name() {
    assert_eq!(math::MathModule.name(), "math");
}

#[test]
fn test_module_function_count() {
    assert_eq!(math::MathModule.functions().len(), 2);
}

#[test]
fn test_module_function_names() {
    let fns = math::MathModule.functions();
    let names: Vec<&str> = fns.iter().map(|f| f.name()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"negate"));
}

#[tokio::test]
async fn test_add_via_module() {
    let fns = math::MathModule.functions();
    let add = fns.iter().find(|f| f.name() == "add").unwrap();
    let result = add
        .execute(
            vec![ExpressionValue::integer(3), ExpressionValue::integer(4)],
            &AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_integer().unwrap(), 7);
}

#[tokio::test]
async fn test_negate_via_module() {
    let fns = math::MathModule.functions();
    let negate = fns.iter().find(|f| f.name() == "negate").unwrap();
    let result = negate
        .execute(
            vec![ExpressionValue::boolean(true)],
            &AgentHandle::detached(),
        )
        .await
        .unwrap();
    assert_eq!(result.as_boolean().unwrap(), false);
}

#[test]
fn test_add_documentation() {
    let fns = math::MathModule.functions();
    let add = fns.iter().find(|f| f.name() == "add").unwrap();
    assert!(add.documentation().is_some());
    assert!(add.documentation().unwrap().contains("Add two integers"));
}
