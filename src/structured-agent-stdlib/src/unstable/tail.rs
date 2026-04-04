use structured_agent_macros::sa_fn;

#[sa_fn(type_params = "T")]
fn tail<T: Clone>(list: Vec<T>) -> Option<Vec<T>> {
    if list.is_empty() {
        None
    } else {
        Some(list[1..].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::TailFunction;
    use arrow::array::{Array, ListBuilder, StringArray, StringBuilder};
    use std::sync::Arc;
    use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFunction};

    #[tokio::test]
    async fn test_tail_properties() {
        let f = TailFunction::new();
        assert_eq!(f.name(), "tail");
        assert_eq!(f.parameters().len(), 1);
        assert_eq!(f.parameters()[0].name, "list");
        assert_eq!(f.return_type().name(), "Option<List<T>>");
        assert_eq!(f.type_params(), &["T"]);
    }

    #[tokio::test]
    async fn test_tail_multiple_elements() {
        let f = TailFunction::new();
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("first");
        builder.values().append_value("second");
        builder.values().append_value("third");
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.execute(args, &AgentHandle::detached()).await.unwrap();
        let inner = result.as_option().unwrap().unwrap();
        let tail_list = inner.as_list().unwrap();
        assert_eq!(tail_list.len(), 1);
        let tail_values = tail_list.value(0);
        let tail_strings = tail_values.as_any().downcast_ref::<StringArray>().unwrap();
        assert_eq!(tail_strings.len(), 2);
        assert_eq!(tail_strings.value(0), "second");
        assert_eq!(tail_strings.value(1), "third");
    }

    #[tokio::test]
    async fn test_tail_single_element() {
        let f = TailFunction::new();
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("only");
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.execute(args, &AgentHandle::detached()).await.unwrap();
        let inner = result.as_option().unwrap().unwrap();
        let elements = inner.as_list_elements().unwrap();
        assert_eq!(elements.len(), 0);
    }

    #[tokio::test]
    async fn test_tail_empty_list() {
        let f = TailFunction::new();
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.execute(args, &AgentHandle::detached()).await.unwrap();
        assert!(result.as_option().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_tail_wrong_type() {
        let f = TailFunction::new();
        let args = vec![ExpressionValue::string("not a list")];
        let result = f.execute(args, &AgentHandle::detached()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tail_wrong_arg_count() {
        let f = TailFunction::new();
        let result = f.execute(vec![], &AgentHandle::detached()).await;
        assert!(result.is_err());
    }
}
