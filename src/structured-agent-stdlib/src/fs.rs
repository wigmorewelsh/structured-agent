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
        use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFunction};

        #[test]
        fn test_get_working_dir_properties() {
            let f = GetWorkingDirFunction::new();
            assert_eq!(f.name(), "get_working_dir");
            assert_eq!(f.parameters().len(), 0);
            assert_eq!(f.return_type().name(), "String");
        }

        #[test]
        fn test_set_working_dir_properties() {
            let f = SetWorkingDirFunction::new();
            assert_eq!(f.name(), "set_working_dir");
            assert_eq!(f.parameters().len(), 1);
            assert_eq!(f.parameters()[0].name, "path");
            assert_eq!(f.return_type().name(), "Unit");
        }

        #[tokio::test]
        async fn test_get_working_dir_returns_string() {
            let f = GetWorkingDirFunction::new();
            let result = f.execute(vec![], &AgentHandle::detached()).await.unwrap();
            assert!(!result.value_string().is_empty());
        }

        #[tokio::test]
        async fn test_get_working_dir_wrong_args() {
            let f = GetWorkingDirFunction::new();
            let result = f
                .execute(vec![ExpressionValue::string("x")], &AgentHandle::detached())
                .await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("get_working_dir expects"));
        }

        #[tokio::test]
        async fn test_set_working_dir_wrong_args() {
            let f = SetWorkingDirFunction::new();
            let result = f.execute(vec![], &AgentHandle::detached()).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("set_working_dir expects"));
        }

        #[tokio::test]
        async fn test_set_working_dir_roundtrip() {
            let original = std::env::current_dir().unwrap();
            let get_fn = GetWorkingDirFunction::new();
            let set_fn = SetWorkingDirFunction::new();

            let current = get_fn
                .execute(vec![], &AgentHandle::detached())
                .await
                .unwrap()
                .value_string();

            let result = set_fn
                .execute(
                    vec![ExpressionValue::string(&current)],
                    &AgentHandle::detached(),
                )
                .await
                .unwrap();
            assert_eq!(result, ExpressionValue::unit());
            std::env::set_current_dir(original).unwrap();
        }

        #[tokio::test]
        async fn test_set_working_dir_invalid_path() {
            let f = SetWorkingDirFunction::new();
            let result = f
                .execute(
                    vec![ExpressionValue::string(
                        "/nonexistent/path/that/does/not/exist",
                    )],
                    &AgentHandle::detached(),
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
