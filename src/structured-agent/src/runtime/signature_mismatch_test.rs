use super::*;
use crate::cli::config::ProgramSource;
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, FunctionProvider, Parameter, Type,
};
use async_trait::async_trait;
use std::sync::Arc;

struct WrongParamNameProvider;

#[async_trait]
impl FunctionProvider for WrongParamNameProvider {
    async fn list_functions(&self) -> Result<Vec<ExternalFunctionDefinition>, RuntimeError> {
        Ok(vec![ExternalFunctionDefinition::new(
            "log".to_string(),
            vec![Parameter::new(
                "wrong_param_name".to_string(),
                Type::string(),
            )],
            Type::unit(),
        )])
    }

    async fn create_expression(
        &self,
        _definition: &ExternalFunctionDefinition,
    ) -> Result<Arc<dyn ExecutableFunction>, RuntimeError> {
        Err(RuntimeError::FunctionNotFound("unreachable".to_string()))
    }
}

struct WrongParamTypeProvider;

#[async_trait]
impl FunctionProvider for WrongParamTypeProvider {
    async fn list_functions(&self) -> Result<Vec<ExternalFunctionDefinition>, RuntimeError> {
        Ok(vec![ExternalFunctionDefinition::new(
            "log".to_string(),
            vec![Parameter::new("message".to_string(), Type::boolean())],
            Type::unit(),
        )])
    }

    async fn create_expression(
        &self,
        _definition: &ExternalFunctionDefinition,
    ) -> Result<Arc<dyn ExecutableFunction>, RuntimeError> {
        Err(RuntimeError::FunctionNotFound("unreachable".to_string()))
    }
}

struct WrongReturnTypeProvider;

#[async_trait]
impl FunctionProvider for WrongReturnTypeProvider {
    async fn list_functions(&self) -> Result<Vec<ExternalFunctionDefinition>, RuntimeError> {
        Ok(vec![ExternalFunctionDefinition::new(
            "log".to_string(),
            vec![Parameter::new("message".to_string(), Type::string())],
            Type::string(),
        )])
    }

    async fn create_expression(
        &self,
        _definition: &ExternalFunctionDefinition,
    ) -> Result<Arc<dyn ExecutableFunction>, RuntimeError> {
        Err(RuntimeError::FunctionNotFound("unreachable".to_string()))
    }
}

#[tokio::test]
async fn test_signature_mismatch_error_message() {
    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    log("test")!
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program_source.to_string()))
        .with_provider(Arc::new(WrongParamNameProvider))
        .build();

    let result = runtime.run().await;

    assert!(result.is_err());

    let error_msg = format!("{:?}", result.unwrap_err());

    assert!(error_msg.contains("No matching provider found for extern function 'log'"));
    assert!(error_msg.contains("Expected signature:"));
    assert!(error_msg.contains("message: String"));
    assert!(error_msg.contains("Available signatures from providers:"));
    assert!(error_msg.contains("wrong_param_name: String"));
}

#[tokio::test]
async fn test_wrong_parameter_type_error_message() {
    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    log("test")!
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program_source.to_string()))
        .with_provider(Arc::new(WrongParamTypeProvider))
        .build();

    let result = runtime.run().await;

    assert!(result.is_err());

    let error_msg = format!("{:?}", result.unwrap_err());

    assert!(error_msg.contains("message: String"));
    assert!(error_msg.contains("message: Boolean"));
}

#[tokio::test]
async fn test_wrong_return_type_error_message() {
    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    log("test")!
}
"#;

    let runtime = Runtime::builder(ProgramSource::Inline(program_source.to_string()))
        .with_provider(Arc::new(WrongReturnTypeProvider))
        .build();

    let result = runtime.run().await;

    assert!(result.is_err());

    let error_msg = format!("{:?}", result.unwrap_err());

    assert!(error_msg.contains("-> Unit"));
    assert!(error_msg.contains("-> String"));
}
