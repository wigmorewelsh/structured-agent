use structured_agent_il::{Instruction, NativeFunctionDef, Slot};
use structured_agent_runtime::{ExpressionValue, NativeFnPtr, Parameter, Type};

pub fn tail_native_def() -> NativeFunctionDef {
    NativeFunctionDef::new(
        "tail".to_string(),
        vec![Parameter::new(
            "list".to_string(),
            Type::list(Type::generic("T")),
        )],
        Type::union(vec![Type::list(Type::generic("T")), Type::unit()]),
        vec!["T".to_string()],
        None,
        vec![
            Instruction::CallNative {
                f: NativeFnPtr::new(|args, _agent| {
                    Box::pin(async move {
                        if args.len() != 1 {
                            return Err(format!("tail expects 1 argument(s), got {}", args.len()));
                        }
                        let elements = args[0].as_list_elements().map_err(|e| e)?;
                        if elements.is_empty() {
                            return Ok(ExpressionValue::unit());
                        }
                        ExpressionValue::from_elements(elements[1..].to_vec()).map_err(|e| e)
                    })
                }),
                params: vec![Slot(2)],
                dest: Slot(0),
            },
            Instruction::Ret { var: Slot(0) },
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::tail_native_def;
    use arrow::array::{Array, ListBuilder, StringArray, StringBuilder};
    use std::sync::Arc;
    use structured_agent_il::Instruction;
    use structured_agent_runtime::{AgentHandle, ExpressionValue, Type};

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
    async fn test_tail_properties() {
        let def = tail_native_def();
        assert_eq!(def.name, "tail");
        assert_eq!(def.parameters.len(), 1);
        assert_eq!(def.parameters[0].name, "list");
        assert_eq!(def.return_type.name(), "List<T> | Unit");
        assert_eq!(def.type_params, &["T"]);
    }

    #[tokio::test]
    async fn tail_return_type_is_union() {
        let def = tail_native_def();
        assert_eq!(
            def.return_type,
            Type::union(vec![Type::list(Type::generic("T")), Type::unit()])
        );
    }

    #[tokio::test]
    async fn test_tail_multiple_elements() {
        let def = tail_native_def();
        let f = get_fn_ptr(&def);
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("first");
        builder.values().append_value("second");
        builder.values().append_value("third");
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.call(args, AgentHandle::detached()).await.unwrap();
        let tail_list = result.as_list().unwrap();
        assert_eq!(tail_list.len(), 1);
        let tail_values = tail_list.value(0);
        let tail_strings = tail_values.as_any().downcast_ref::<StringArray>().unwrap();
        assert_eq!(tail_strings.len(), 2);
        assert_eq!(tail_strings.value(0), "second");
        assert_eq!(tail_strings.value(1), "third");
    }

    #[tokio::test]
    async fn test_tail_single_element() {
        let def = tail_native_def();
        let f = get_fn_ptr(&def);
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.values().append_value("only");
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.call(args, AgentHandle::detached()).await.unwrap();
        let elements = result.as_list_elements().unwrap();
        assert_eq!(elements.len(), 0);
    }

    #[tokio::test]
    async fn test_tail_empty_list() {
        let def = tail_native_def();
        let f = get_fn_ptr(&def);
        let mut builder = ListBuilder::new(StringBuilder::new());
        builder.append(true);
        let list_array = Arc::new(builder.finish());
        let args = vec![ExpressionValue::list(list_array)];
        let result = f.call(args, AgentHandle::detached()).await.unwrap();
        assert_eq!(result.type_name(), "Unit");
    }

    #[tokio::test]
    async fn test_tail_wrong_type() {
        let def = tail_native_def();
        let f = get_fn_ptr(&def);
        let args = vec![ExpressionValue::string("not a list")];
        let result = f.call(args, AgentHandle::detached()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tail_wrong_arg_count() {
        let def = tail_native_def();
        let f = get_fn_ptr(&def);
        let result = f.call(vec![], AgentHandle::detached()).await;
        assert!(result.is_err());
    }
}
