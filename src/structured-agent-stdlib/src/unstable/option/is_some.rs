use structured_agent_macros::sa_fn;

#[sa_fn(type_params = "T")]
fn is_some<T>(value: Option<T>) -> bool {
    value.is_some()
}

#[cfg(test)]
mod tests {
    use super::is_some_native_def;
    use structured_agent_il::Instruction;
    use structured_agent_runtime::{AgentHandle, ExpressionValue};

    fn get_fn_ptr(
        def: &structured_agent_il::NativeFunctionDef,
    ) -> structured_agent_runtime::NativeFnPtr {
        if let Instruction::CallNative { f, .. } = &def.body[0] {
            f.clone()
        } else {
            panic!("expected CallNative instruction");
        }
    }

    #[tokio::test]
    async fn test_is_some_properties() {
        let def = is_some_native_def();
        assert_eq!(def.name, "is_some");
        assert_eq!(def.parameters.len(), 1);
        assert_eq!(def.parameters[0].name, "value");
        assert_eq!(def.return_type.name(), "Boolean");
        assert_eq!(def.type_params, &["T"]);
    }

    #[tokio::test]
    async fn test_is_some_with_some() {
        let def = is_some_native_def();
        let f = get_fn_ptr(&def);
        let result = f
            .call(
                vec![ExpressionValue::option_some(ExpressionValue::string("val"))],
                AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_boolean().unwrap(), true);
    }

    #[tokio::test]
    async fn test_is_some_with_none() {
        let def = is_some_native_def();
        let f = get_fn_ptr(&def);
        let result = f
            .call(
                vec![ExpressionValue::option_none()],
                AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_boolean().unwrap(), false);
    }

    #[tokio::test]
    async fn test_is_some_wrong_type() {
        let def = is_some_native_def();
        let f = get_fn_ptr(&def);
        let result = f
            .call(
                vec![ExpressionValue::string("not an option")],
                AgentHandle::detached(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_some_wrong_arg_count() {
        let def = is_some_native_def();
        let f = get_fn_ptr(&def);
        let result = f.call(vec![], AgentHandle::detached()).await;
        assert!(result.is_err());
    }
}
