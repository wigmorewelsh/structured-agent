use super::*;
use crate::mcp::McpClient;
use crate::runtime::ExprResult;
use crate::types::{ExternalFunctionDefinition, Type};
use tokio;

#[tokio::test]
async fn test_runtime_builder_with_mcp_client() {
    let client = McpClient::new_stdio("echo", vec![]).await.unwrap();
    let runtime = Runtime::builder().with_mcp_client(client).build();

    // Check that runtime was created successfully with MCP client
    assert_eq!(runtime.mcp_clients_count(), 1);
}

#[tokio::test]
async fn test_runtime_builder_with_multiple_mcp_clients() {
    let client1 = McpClient::new_stdio("echo", vec![]).await.unwrap();
    let client2 = McpClient::new_stdio("cat", vec![]).await.unwrap();
    let clients = vec![client1, client2];

    let runtime = Runtime::builder().with_mcp_clients(clients).build();

    // Test that runtime was created successfully with multiple clients
    assert_eq!(runtime.mcp_clients_count(), 2);
}

#[test]
fn test_external_function_registration() {
    let mut runtime = Runtime::new();

    let ext_func = ExternalFunctionDefinition::new(
        "test_tool".to_string(),
        vec![("param1".to_string(), Type::string())],
        Type::string(),
    );

    runtime.register_external_function(ext_func);

    assert!(runtime.get_external_function("test_tool").is_some());
}

#[test]
fn test_function_registry_with_executable_functions() {
    let runtime = Runtime::new();

    // Test that function registry starts empty
    assert!(runtime.get_function("nonexistent").is_none());

    // Test that we can list functions
    let functions = runtime.list_functions();
    assert_eq!(functions.len(), 0);
}

#[test]
fn test_runtime_clone_creates_separate_instance() {
    let runtime = Runtime::new();
    let cloned = runtime.clone();

    // Both should be separate instances
    assert!(runtime.get_function("test").is_none());
    assert!(cloned.get_function("test").is_none());
}

#[test]
fn test_register_native_function_sync() {
    use crate::types::NativeFunction;
    use async_trait::async_trait;

    // Define a simple print function using the trait directly
    #[derive(Debug)]
    struct PrintFunction {
        parameters: Vec<(String, Type)>,
        return_type: Type,
    }

    impl PrintFunction {
        fn new() -> Self {
            Self {
                parameters: vec![("message".to_string(), Type::string())],
                return_type: Type::unit(),
            }
        }
    }

    #[async_trait(?Send)]
    impl NativeFunction for PrintFunction {
        fn name(&self) -> &str {
            "print"
        }

        fn parameters(&self) -> &[(String, Type)] {
            &self.parameters
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
            if let Some(ExprResult::String(message)) = args.get(0) {
                println!("PRINT: {}", message);
                Ok(ExprResult::Unit)
            } else {
                Err("Expected string argument for print".to_string())
            }
        }
    }

    let mut runtime = Runtime::new();
    runtime.register_native_function(Box::new(PrintFunction::new()));

    // Verify the function was registered
    assert!(runtime.get_function("print").is_some());
}

#[tokio::test]
async fn test_register_native_function_async() {
    use crate::types::NativeFunction;
    use async_trait::async_trait;

    // Define an async logging function
    #[derive(Debug)]
    struct LogFunction {
        parameters: Vec<(String, Type)>,
        return_type: Type,
    }

    impl LogFunction {
        fn new() -> Self {
            Self {
                parameters: vec![
                    ("level".to_string(), Type::string()),
                    ("message".to_string(), Type::string()),
                ],
                return_type: Type::unit(),
            }
        }
    }

    #[async_trait(?Send)]
    impl NativeFunction for LogFunction {
        fn name(&self) -> &str {
            "log"
        }

        fn parameters(&self) -> &[(String, Type)] {
            &self.parameters
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
            if args.len() >= 2 {
                if let (Some(ExprResult::String(level)), Some(ExprResult::String(message))) =
                    (args.get(0), args.get(1))
                {
                    // Simulate async processing
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    println!("LOG [{}]: {}", level, message);
                    Ok(ExprResult::Unit)
                } else {
                    Err("Expected string arguments for log".to_string())
                }
            } else {
                Err("Expected 2 arguments for log function".to_string())
            }
        }
    }

    let mut runtime = Runtime::new();
    runtime.register_native_function(Box::new(LogFunction::new()));

    // Verify the function was registered
    assert!(runtime.get_function("log").is_some());
}

#[test]
fn test_runtime_builder_with_native_functions() {
    use crate::types::NativeFunction;
    use async_trait::async_trait;

    // Define an add function using the trait
    #[derive(Debug)]
    struct AddFunction {
        parameters: Vec<(String, Type)>,
        return_type: Type,
    }

    impl AddFunction {
        fn new() -> Self {
            Self {
                parameters: vec![
                    ("a".to_string(), Type::string()),
                    ("b".to_string(), Type::string()),
                ],
                return_type: Type::string(),
            }
        }
    }

    #[async_trait(?Send)]
    impl NativeFunction for AddFunction {
        fn name(&self) -> &str {
            "add"
        }

        fn parameters(&self) -> &[(String, Type)] {
            &self.parameters
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
            if args.len() >= 2 {
                if let (Some(ExprResult::String(a)), Some(ExprResult::String(b))) =
                    (args.get(0), args.get(1))
                {
                    if let (Ok(num_a), Ok(num_b)) = (a.parse::<i32>(), b.parse::<i32>()) {
                        Ok(ExprResult::String((num_a + num_b).to_string()))
                    } else {
                        Err("Arguments must be valid numbers".to_string())
                    }
                } else {
                    Err("Expected string arguments".to_string())
                }
            } else {
                Err("Expected 2 arguments".to_string())
            }
        }
    }

    let runtime = Runtime::builder()
        .with_native_function(Box::new(AddFunction::new()))
        .build();

    // Verify the function was registered
    assert!(runtime.get_function("add").is_some());
}

#[test]
fn test_register_native_function_trait() {
    use crate::types::NativeFunction;
    use async_trait::async_trait;

    // Define a custom native function using the trait
    #[derive(Debug)]
    struct CustomPrintFunction {
        parameters: Vec<(String, Type)>,
        return_type: Type,
    }

    impl CustomPrintFunction {
        fn new() -> Self {
            Self {
                parameters: vec![("message".to_string(), Type::string())],
                return_type: Type::unit(),
            }
        }
    }

    #[async_trait(?Send)]
    impl NativeFunction for CustomPrintFunction {
        fn name(&self) -> &str {
            "custom_print"
        }

        fn parameters(&self) -> &[(String, Type)] {
            &self.parameters
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
            if let Some(ExprResult::String(message)) = args.get(0) {
                println!("CUSTOM PRINT: {}", message);
                Ok(ExprResult::Unit)
            } else {
                Err("Expected string argument".to_string())
            }
        }
    }

    let mut runtime = Runtime::new();

    // Register using the trait directly
    runtime.register_native_function(Box::new(CustomPrintFunction::new()));

    // Verify the function was registered
    assert!(runtime.get_function("custom_print").is_some());

    let func = runtime.get_function("custom_print").unwrap();
    assert_eq!(func.name(), "custom_print");
    assert_eq!(func.parameters().len(), 1);
    assert_eq!(func.parameters()[0].0, "message");
}
