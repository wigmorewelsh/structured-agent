use structured_agent_macros::sa_fn;

#[sa_fn(type_params = "T")]
fn head<T: Clone>(list: Vec<T>) -> Option<T> {
    list.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::head_native_def;
    use arrow::array::{ListBuilder, StringBuilder};
    use std::sync::Arc;
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
    async fn test_head_properties() {
        let def = head_native_def();
        assert_eq!(def.name, "head");
        assert_eq!(def.parameters.len(), 1);
        assert_eq!(def.parameters[0].name, "list");
        assert_eq!(def.return_type.name(), "Option<T>");
        assert_eq!(def.type_params, &["T"]);
    }

    #[tokio::test]
    async fn test_head_non_empty_list() {
        let def = head_native_def();
        let f = get_fn_ptr(&def);
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("first");
        builder.values().append_value("second");
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.call(args, AgentHandle::detached()).await.unwrap();
        let inner = result.as_option().unwrap().unwrap();
        assert_eq!(inner.as_string().unwrap(), "first");
    }

    #[tokio::test]
    async fn test_head_empty_list() {
        let def = head_native_def();
        let f = get_fn_ptr(&def);
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.call(args, AgentHandle::detached()).await.unwrap();
        assert!(result.as_option().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_head_wrong_type() {
        let def = head_native_def();
        let f = get_fn_ptr(&def);
        let args = vec![ExpressionValue::string("not a list")];
        let result = f.call(args, AgentHandle::detached()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_head_wrong_arg_count() {
        let def = head_native_def();
        let f = get_fn_ptr(&def);
        let result = f.call(vec![], AgentHandle::detached()).await;
        assert!(result.is_err());
    }
}
