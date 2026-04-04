use structured_agent_macros::sa_fn;

#[sa_fn(type_params = "T")]
fn head<T: Clone>(list: Vec<T>) -> Option<T> {
    list.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::HeadFunction;
    use arrow::array::{ListBuilder, StringBuilder};
    use std::sync::Arc;
    use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFunction};

    #[tokio::test]
    async fn test_head_properties() {
        let f = HeadFunction::new();
        assert_eq!(f.name(), "head");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(f.parameters()[0].name, "list");
        assert_eq!(f.return_type().name(), "Option<T>");
        assert_eq!(f.type_params(), &["T"]);
    }

    #[tokio::test]
    async fn test_head_non_empty_list() {
        let f = HeadFunction::new();
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("first");
        builder.values().append_value("second");
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.execute(args, &AgentHandle::detached()).await.unwrap();
        let inner = result.as_option().unwrap().unwrap();
        assert_eq!(inner.as_string().unwrap(), "first");
    }

    #[tokio::test]
    async fn test_head_empty_list() {
        let f = HeadFunction::new();
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.execute(args, &AgentHandle::detached()).await.unwrap();
        assert!(result.as_option().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_head_wrong_type() {
        let f = HeadFunction::new();
        let args = vec![ExpressionValue::string("not a list")];
        let result = f.execute(args, &AgentHandle::detached()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_head_wrong_arg_count() {
        let f = HeadFunction::new();
        let result = f.execute(vec![], &AgentHandle::detached()).await;
        assert!(result.is_err());
    }
}
