use structured_agent_macros::sa_fn;

#[sa_fn(type_params = "T")]
fn some_value<T>(value: Option<T>) -> T {
    value.ok_or_else(|| "some_value called on None".to_string())?
}

#[cfg(test)]
mod tests {
    use super::SomeValueFunction;
    use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFunction};

    #[tokio::test]
    async fn test_some_value_properties() {
        let f = SomeValueFunction::new();
        assert_eq!(f.name(), "some_value");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(f.parameters()[0].name, "value");
        assert_eq!(f.return_type().name(), "T");
        assert_eq!(f.type_params(), &["T"]);
    }

    #[tokio::test]
    async fn test_some_value_with_some_string() {
        let f = SomeValueFunction::new();
        let result = f
            .execute(
                vec![ExpressionValue::option_some(ExpressionValue::string(
                    "test_value",
                ))],
                &AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_string().unwrap(), "test_value");
    }

    #[tokio::test]
    async fn test_some_value_with_none() {
        let f = SomeValueFunction::new();
        let result = f
            .execute(
                vec![ExpressionValue::option_none()],
                &AgentHandle::detached(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_some_value_wrong_type() {
        let f = SomeValueFunction::new();
        let result = f
            .execute(
                vec![ExpressionValue::string("not an option")],
                &AgentHandle::detached(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_some_value_wrong_arg_count() {
        let f = SomeValueFunction::new();
        let result = f.execute(vec![], &AgentHandle::detached()).await;
        assert!(result.is_err());
    }
}
