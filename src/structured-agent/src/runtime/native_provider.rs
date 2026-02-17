use crate::expressions::NativeFunctionExpr;
use crate::runtime::RuntimeError;
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, Function, FunctionProvider, NativeFunction,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

pub struct NativeFunctionProvider {
    pub(crate) native_functions: HashMap<String, Arc<dyn NativeFunction>>,
}

impl NativeFunctionProvider {
    pub fn new() -> Self {
        Self {
            native_functions: HashMap::new(),
        }
    }

    pub fn add_function(&mut self, native_function: Arc<dyn NativeFunction>) {
        let name = native_function.name().to_string();
        self.native_functions.insert(name, native_function);
    }
}

impl Default for NativeFunctionProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl FunctionProvider for NativeFunctionProvider {
    async fn list_functions(&self) -> Result<Vec<ExternalFunctionDefinition>, RuntimeError> {
        let definitions = self
            .native_functions
            .values()
            .map(|native_fn| {
                ExternalFunctionDefinition::new_with_docs(
                    native_fn.name().to_string(),
                    native_fn.parameters().to_vec(),
                    native_fn.return_type().clone(),
                    native_fn.documentation().map(|s| s.to_string()),
                )
            })
            .collect();
        Ok(definitions)
    }

    async fn create_expression(
        &self,
        definition: &ExternalFunctionDefinition,
    ) -> Result<Rc<dyn ExecutableFunction>, RuntimeError> {
        let native_function = self.native_functions.get(&definition.name).ok_or_else(|| {
            RuntimeError::FunctionNotFound(format!(
                "Native function '{}' not found",
                definition.name
            ))
        })?;

        let expr = NativeFunctionExpr::new(native_function.clone());
        Ok(Rc::new(expr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ExpressionValue;
    use crate::types::{Parameter, Type};

    #[derive(Debug)]
    struct TestNativeFunction {
        name: String,
        parameters: Vec<Parameter>,
        return_type: Type,
    }

    impl TestNativeFunction {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                parameters: vec![Parameter::new("arg".to_string(), Type::string())],
                return_type: Type::string(),
            }
        }
    }

    #[async_trait(?Send)]
    impl NativeFunction for TestNativeFunction {
        fn name(&self) -> &str {
            &self.name
        }

        fn parameters(&self) -> &[Parameter] {
            &self.parameters
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(&self, _args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
            Ok(ExpressionValue::String("test_result".to_string()))
        }

        fn documentation(&self) -> Option<&str> {
            Some("Test function documentation")
        }
    }

    #[tokio::test]
    async fn test_native_provider_list_functions() {
        let mut provider = NativeFunctionProvider::new();
        provider.add_function(Arc::new(TestNativeFunction::new("test_fn")));

        let functions = provider.list_functions().await.unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "test_fn");
        assert_eq!(functions[0].parameters.len(), 1);
        assert_eq!(functions[0].return_type, Type::string());
        assert_eq!(
            functions[0].documentation,
            Some("Test function documentation".to_string())
        );
    }

    #[tokio::test]
    async fn test_native_provider_create_expression() {
        let mut provider = NativeFunctionProvider::new();
        let native_fn = Arc::new(TestNativeFunction::new("test_fn"));
        provider.add_function(native_fn);

        let definition = ExternalFunctionDefinition::new(
            "test_fn".to_string(),
            vec![Parameter::new("arg".to_string(), Type::string())],
            Type::string(),
        );

        let expr = provider.create_expression(&definition).await.unwrap();
        assert_eq!(Function::name(expr.as_ref()), "test_fn");
    }

    #[tokio::test]
    async fn test_native_provider_create_expression_not_found() {
        let provider = NativeFunctionProvider::new();

        let definition =
            ExternalFunctionDefinition::new("nonexistent".to_string(), vec![], Type::string());

        let result = provider.create_expression(&definition).await;
        assert!(result.is_err());
        match result {
            Err(RuntimeError::FunctionNotFound(msg)) => {
                assert!(msg.contains("nonexistent"));
            }
            _ => panic!("Expected FunctionNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_native_provider_add_multiple_functions() {
        let mut provider = NativeFunctionProvider::new();
        provider.add_function(Arc::new(TestNativeFunction::new("fn1")));
        provider.add_function(Arc::new(TestNativeFunction::new("fn2")));

        let functions = provider.list_functions().await.unwrap();
        assert_eq!(functions.len(), 2);

        let names: Vec<_> = functions.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"fn1"));
        assert!(names.contains(&"fn2"));
    }
}
