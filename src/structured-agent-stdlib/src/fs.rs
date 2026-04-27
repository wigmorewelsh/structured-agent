use structured_agent_macros::sa_module;

#[sa_module]
pub mod fs {
    #[sa_fn]
    async fn get_working_dir() -> String {
        let path = std::env::current_dir()
            .map_err(|e| format!("Failed to get working directory: {}", e))?;
        path.to_string_lossy().into_owned()
    }

    #[sa_fn]
    async fn set_working_dir(path: String) {
        std::env::set_current_dir(&path)
            .map_err(|e| format!("Failed to set working directory to '{}': {}", path, e))?;
    }

    #[cfg(test)]
    mod tests {
        use super::*;
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

        #[test]
        fn test_get_working_dir_properties() {
            let def = get_working_dir_native_def();
            assert_eq!(def.name, "get_working_dir");
            assert_eq!(def.parameters.len(), 0);
            assert_eq!(def.return_type.name(), "String");
        }

        #[test]
        fn test_set_working_dir_properties() {
            let def = set_working_dir_native_def();
            assert_eq!(def.name, "set_working_dir");
            assert_eq!(def.parameters.len(), 1);
            assert_eq!(def.parameters[0].name, "path");
            assert_eq!(def.return_type.name(), "Unit");
        }

        #[tokio::test]
        async fn test_get_working_dir_returns_string() {
            let def = get_working_dir_native_def();
            let f = get_fn_ptr(&def);
            let result = f.call(vec![], AgentHandle::detached()).await.unwrap();
            assert!(!result.value_string().is_empty());
        }

        #[tokio::test]
        async fn test_get_working_dir_wrong_args() {
            let def = get_working_dir_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(vec![ExpressionValue::string("x")], AgentHandle::detached())
                .await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("get_working_dir expects"));
        }

        #[tokio::test]
        async fn test_set_working_dir_wrong_args() {
            let def = set_working_dir_native_def();
            let f = get_fn_ptr(&def);
            let result = f.call(vec![], AgentHandle::detached()).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("set_working_dir expects"));
        }

        #[tokio::test]
        async fn test_set_working_dir_roundtrip() {
            let original = std::env::current_dir().unwrap();
            let get_def = get_working_dir_native_def();
            let set_def = set_working_dir_native_def();
            let get_f = get_fn_ptr(&get_def);
            let set_f = get_fn_ptr(&set_def);

            let current = get_f
                .call(vec![], AgentHandle::detached())
                .await
                .unwrap()
                .value_string();

            let result = set_f
                .call(
                    vec![ExpressionValue::string(&current)],
                    AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result, ExpressionValue::unit());
            std::env::set_current_dir(original).unwrap();
        }

        #[tokio::test]
        async fn test_set_working_dir_invalid_path() {
            let def = set_working_dir_native_def();
            let f = get_fn_ptr(&def);
            let result = f
                .call(
                    vec![ExpressionValue::string(
                        "/nonexistent/path/that/does/not/exist",
                    )],
                    AgentHandle::detached(),
                )
                .await;
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .contains("Failed to set working directory")
            );
        }
    }
}

pub use fs::FsModule;
