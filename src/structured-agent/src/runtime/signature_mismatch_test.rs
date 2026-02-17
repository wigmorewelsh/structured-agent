use super::*;
use crate::compiler::CompilationUnit;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug)]
struct WrongSignatureFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl WrongSignatureFunction {
    fn new() -> Self {
        Self {
            parameters: vec![Parameter::new(
                "wrong_param_name".to_string(),
                Type::string(),
            )],
            return_type: Type::unit(),
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for WrongSignatureFunction {
    fn name(&self) -> &str {
        "log"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, _args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        Ok(ExpressionValue::Unit)
    }
}

#[tokio::test]
async fn test_signature_mismatch_error_message() {
    let wrong_func = Arc::new(WrongSignatureFunction::new());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    log("test")!
}
"#;

    let runtime = Runtime::builder(CompilationUnit::from_string(program_source.to_string()))
        .with_native_function(wrong_func)
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
    #[derive(Debug)]
    struct WrongTypeFunction {
        parameters: Vec<Parameter>,
        return_type: Type,
    }

    impl WrongTypeFunction {
        fn new() -> Self {
            Self {
                parameters: vec![Parameter::new("message".to_string(), Type::Boolean)],
                return_type: Type::unit(),
            }
        }
    }

    #[async_trait(?Send)]
    impl NativeFunction for WrongTypeFunction {
        fn name(&self) -> &str {
            "log"
        }

        fn parameters(&self) -> &[Parameter] {
            &self.parameters
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(&self, _args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
            Ok(ExpressionValue::Unit)
        }
    }

    let wrong_func = Arc::new(WrongTypeFunction::new());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    log("test")!
}
"#;

    let runtime = Runtime::builder(CompilationUnit::from_string(program_source.to_string()))
        .with_native_function(wrong_func)
        .build();

    let result = runtime.run().await;

    assert!(result.is_err());

    let error_msg = format!("{:?}", result.unwrap_err());

    assert!(error_msg.contains("message: String"));
    assert!(error_msg.contains("message: Boolean"));
}

#[tokio::test]
async fn test_wrong_return_type_error_message() {
    #[derive(Debug)]
    struct WrongReturnFunction {
        parameters: Vec<Parameter>,
        return_type: Type,
    }

    impl WrongReturnFunction {
        fn new() -> Self {
            Self {
                parameters: vec![Parameter::new("message".to_string(), Type::string())],
                return_type: Type::string(),
            }
        }
    }

    #[async_trait(?Send)]
    impl NativeFunction for WrongReturnFunction {
        fn name(&self) -> &str {
            "log"
        }

        fn parameters(&self) -> &[Parameter] {
            &self.parameters
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(&self, _args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
            Ok(ExpressionValue::String("test".to_string()))
        }
    }

    let wrong_func = Arc::new(WrongReturnFunction::new());

    let program_source = r#"
extern fn log(message: String): ()

fn main(): () {
    log("test")!
}
"#;

    let runtime = Runtime::builder(CompilationUnit::from_string(program_source.to_string()))
        .with_native_function(wrong_func)
        .build();

    let result = runtime.run().await;

    assert!(result.is_err());

    let error_msg = format!("{:?}", result.unwrap_err());

    assert!(error_msg.contains("-> Unit"));
    assert!(error_msg.contains("-> String"));
}
