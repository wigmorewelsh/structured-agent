use structured_agent_il::{Instruction, NativeFunctionDef, Slot};
use structured_agent_runtime::{ExpressionValue, NativeFnPtr, Parameter, Type};

pub fn head_native_def() -> NativeFunctionDef {
    NativeFunctionDef::new(
        "head".to_string(),
        vec![Parameter::new(
            "list".to_string(),
            Type::list(Type::generic("T")),
        )],
        Type::union(vec![Type::generic("T"), Type::unit()]),
        vec!["T".to_string()],
        None,
        vec![
            Instruction::CallNative {
                f: NativeFnPtr::new(|args, _agent| {
                    Box::pin(async move {
                        if args.len() != 1 {
                            return Err(format!("head expects 1 argument(s), got {}", args.len()));
                        }
                        let elements = args[0].as_list_elements().map_err(|e| e)?;
                        Ok(elements
                            .into_iter()
                            .next()
                            .unwrap_or_else(ExpressionValue::unit))
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
    use super::head_native_def;
    use arrow::array::{ListBuilder, StringBuilder};
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
    async fn test_head_properties() {
        let def = head_native_def();
        assert_eq!(def.name, "head");
        assert_eq!(def.parameters.len(), 1);
        assert_eq!(def.parameters[0].name, "list");
        assert_eq!(def.return_type.name(), "T | Unit");
        assert_eq!(def.type_params, &["T"]);
    }

    #[tokio::test]
    async fn head_return_type_is_union() {
        let def = head_native_def();
        assert_eq!(
            def.return_type,
            Type::union(vec![Type::generic("T"), Type::unit()])
        );
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
        assert_eq!(result.as_string().unwrap(), "first");
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
        assert_eq!(result.type_name(), "Unit");
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
