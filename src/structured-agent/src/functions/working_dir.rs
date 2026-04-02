use crate::runtime::{AgentHandle, ExpressionValue};
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;

#[derive(Debug)]
pub struct GetWorkingDirFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for GetWorkingDirFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl GetWorkingDirFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![],
            return_type: Type::string(),
        }
    }
}

#[async_trait]
impl NativeFunction for GetWorkingDirFunction {
    fn name(&self) -> &str {
        "get_working_dir"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(
        &self,
        args: Vec<ExpressionValue>,
        _agent: &AgentHandle,
    ) -> Result<ExpressionValue, String> {
        if !args.is_empty() {
            return Err(format!(
                "get_working_dir expects 0 arguments, got {}",
                args.len()
            ));
        }
        let path = std::env::current_dir()
            .map_err(|e| format!("Failed to get working directory: {}", e))?;
        Ok(ExpressionValue::string(path.to_string_lossy().into_owned()))
    }
}

#[derive(Debug)]
pub struct SetWorkingDirFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for SetWorkingDirFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl SetWorkingDirFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![Parameter::new("path".to_string(), Type::string())],
            return_type: Type::unit(),
        }
    }
}

#[async_trait]
impl NativeFunction for SetWorkingDirFunction {
    fn name(&self) -> &str {
        "set_working_dir"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(
        &self,
        args: Vec<ExpressionValue>,
        _agent: &AgentHandle,
    ) -> Result<ExpressionValue, String> {
        if args.len() != 1 {
            return Err(format!(
                "set_working_dir expects 1 argument, got {}",
                args.len()
            ));
        }
        let path = args[0].value_string();
        std::env::set_current_dir(&path)
            .map_err(|e| format!("Failed to set working directory to '{}': {}", path, e))?;
        Ok(ExpressionValue::unit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(f.return_type().name(), "()");
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
        assert!(result.unwrap_err().contains("get_working_dir expects 0"));
    }

    #[tokio::test]
    async fn test_set_working_dir_wrong_args() {
        let f = SetWorkingDirFunction::new();
        let result = f.execute(vec![], &AgentHandle::detached()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("set_working_dir expects 1"));
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
