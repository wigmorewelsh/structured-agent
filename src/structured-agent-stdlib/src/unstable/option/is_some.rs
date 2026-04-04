use structured_agent_macros::sa_fn;

#[sa_fn(type_params = "T")]
fn is_some<T>(value: Option<T>) -> bool {
    value.is_some()
}

#[cfg(test)]
mod tests {
    use super::IsSomeFunction;
    use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFunction};

    #[tokio::test]
    async fn test_is_some_properties() {
        let f = IsSomeFunction::new();
        assert_eq!(f.name(), "is_some");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(f.parameters()[0].name, "value");
        assert_eq!(f.return_type().name(), "Boolean");
        assert_eq!(f.type_params(), &["T"]);
    }

    #[tokio::test]
    async fn test_is_some_with_some() {
        let f = IsSomeFunction::new();
        let result = f
            .execute(
                vec![ExpressionValue::option_some(ExpressionValue::string("val"))],
                &AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_boolean().unwrap(), true);
    }

    #[tokio::test]
    async fn test_is_some_with_none() {
        let f = IsSomeFunction::new();
        let result = f
            .execute(
                vec![ExpressionValue::option_none()],
                &AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_boolean().unwrap(), false);
    }

    #[tokio::test]
    async fn test_is_some_wrong_type() {
        let f = IsSomeFunction::new();
        let result = f
            .execute(
                vec![ExpressionValue::string("not an option")],
                &AgentHandle::detached(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_some_wrong_arg_count() {
        let f = IsSomeFunction::new();
        let result = f.execute(vec![], &AgentHandle::detached()).await;
        assert!(result.is_err());
    }
}
