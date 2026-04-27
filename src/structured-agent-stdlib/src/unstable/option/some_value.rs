use structured_agent_macros::sa_fn;

#[sa_fn(type_params = "T")]
fn some_value<T>(value: Option<T>) -> T {
    value.ok_or_else(|| "some_value called on None".to_string())?
}

#[cfg(test)]
mod tests {
    use super::some_value_native_def;
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
    async fn test_some_value_properties() {
        let def = some_value_native_def();
        assert_eq!(def.name, "some_value");
        assert_eq!(def.parameters.len(), 1);
        assert_eq!(def.parameters[0].name, "value");
        assert_eq!(def.return_type.name(), "T");
        assert_eq!(def.type_params, &["T"]);
    }

    #[tokio::test]
    async fn test_some_value_with_some_string() {
        let def = some_value_native_def();
        let f = get_fn_ptr(&def);
        let result = f
            .call(
                vec![ExpressionValue::option_some(ExpressionValue::string(
                    "test_value",
                ))],
                AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_string().unwrap(), "test_value");
    }

    #[tokio::test]
    async fn test_some_value_with_none() {
        let def = some_value_native_def();
        let f = get_fn_ptr(&def);
        let result = f
            .call(
                vec![ExpressionValue::option_none()],
                AgentHandle::detached(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_some_value_wrong_type() {
        let def = some_value_native_def();
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
    async fn test_some_value_wrong_arg_count() {
        let def = some_value_native_def();
        let f = get_fn_ptr(&def);
        let result = f.call(vec![], AgentHandle::detached()).await;
        assert!(result.is_err());
    }
}
