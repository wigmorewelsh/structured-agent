use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{ExecutableFunction, Function, NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::any::Any;

use std::sync::Arc;

pub struct NativeFunctionExpr<F: NativeFunction> {
    native_function: Arc<F>,
}

impl<F: NativeFunction> std::fmt::Debug for NativeFunctionExpr<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeFunctionExpr")
            .field("name", &self.native_function.name())
            .field("parameters", &self.native_function.parameters())
            .field("return_type", &self.native_function.return_type())
            .finish()
    }
}

impl<F: NativeFunction> Clone for NativeFunctionExpr<F> {
    fn clone(&self) -> Self {
        panic!("NativeFunctionExpr cannot be cloned due to boxed trait object")
    }
}

#[async_trait]
impl<F: NativeFunction + 'static> Function for NativeFunctionExpr<F> {
    fn name(&self) -> &str {
        self.native_function.name()
    }

    fn parameters(&self) -> &[Parameter] {
        self.native_function.parameters()
    }

    fn function_return_type(&self) -> &Type {
        self.native_function.return_type()
    }

    async fn execute(
        &self,
        context: Context,
        args: Vec<ExpressionResult>,
    ) -> Result<(Context, ExpressionResult), String> {
        let values: Vec<ExpressionValue> = args.into_iter().map(|r| r.value).collect();
        let result = self.native_function.execute(values).await?;
        Ok((context, ExpressionResult::new(result)))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Function> {
        panic!("NativeFunctionExpr cannot be cloned due to boxed trait object")
    }

    fn documentation(&self) -> Option<&str> {
        self.native_function.documentation()
    }
}

#[async_trait]
impl<F: NativeFunction + 'static> ExecutableFunction for NativeFunctionExpr<F> {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        panic!("NativeFunctionExpr cannot be cloned due to boxed trait object")
    }
}

impl<F: NativeFunction> NativeFunctionExpr<F> {
    pub fn new(native_function: Arc<F>) -> Self {
        Self { native_function }
    }
}

pub fn create_native_function_expr<F: NativeFunction + 'static>(
    native_function: Arc<F>,
) -> Arc<dyn ExecutableFunction> {
    Arc::new(NativeFunctionExpr::new(native_function))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompilationUnit;
    use crate::runtime::{Context, Runtime};
    use crate::types::{NativeFunction, Type};

    fn test_runtime() -> Runtime {
        let program = CompilationUnit::from_string("fn main(): () {}".to_string());
        Runtime::builder(program).build()
    }

    #[derive(Debug)]
    struct TestNativeFunctionWithDocs {
        return_type: Type,
    }

    impl TestNativeFunctionWithDocs {
        fn new() -> Self {
            Self {
                return_type: Type::string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl NativeFunction for TestNativeFunctionWithDocs {
        fn name(&self) -> &str {
            "test_function"
        }

        fn parameters(&self) -> &[Parameter] {
            &[]
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(
            &self,
            _args: Vec<crate::runtime::ExpressionValue>,
        ) -> Result<crate::runtime::ExpressionValue, String> {
            Ok(crate::runtime::ExpressionValue::String(
                "test_result".to_string(),
            ))
        }

        fn documentation(&self) -> Option<&str> {
            Some("This is a test function with documentation")
        }
    }

    #[derive(Debug)]
    struct TestNativeFunctionWithoutDocs {
        return_type: Type,
    }

    impl TestNativeFunctionWithoutDocs {
        fn new() -> Self {
            Self {
                return_type: Type::string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl NativeFunction for TestNativeFunctionWithoutDocs {
        fn name(&self) -> &str {
            "undocumented_function"
        }

        fn parameters(&self) -> &[Parameter] {
            &[]
        }

        fn return_type(&self) -> &Type {
            &self.return_type
        }

        async fn execute(
            &self,
            _args: Vec<crate::runtime::ExpressionValue>,
        ) -> Result<crate::runtime::ExpressionValue, String> {
            Ok(crate::runtime::ExpressionValue::String(
                "undocumented_result".to_string(),
            ))
        }
    }

    #[test]
    fn test_native_function_with_documentation() {
        let native_func = Arc::new(TestNativeFunctionWithDocs::new());
        let expr = NativeFunctionExpr::new(native_func);

        assert_eq!(
            expr.documentation(),
            Some("This is a test function with documentation")
        );
    }

    #[test]
    fn test_native_function_without_documentation() {
        let native_func = Arc::new(TestNativeFunctionWithoutDocs::new());
        let expr = NativeFunctionExpr::new(native_func);

        assert_eq!(expr.documentation(), None);
    }

    #[tokio::test]
    async fn test_native_function_evaluation_with_docs() {
        let native_func = Arc::new(TestNativeFunctionWithDocs::new());
        let expr = NativeFunctionExpr::new(native_func);

        let runtime = Arc::new(test_runtime());
        let context = Context::with_runtime(runtime);

        let (_context, result) = expr.execute(context, vec![]).await.unwrap();

        match result.value {
            crate::runtime::ExpressionValue::String(s) => assert_eq!(s, "test_result"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(
            expr.documentation(),
            Some("This is a test function with documentation")
        );
    }
}
