use crate::runtime::{Context, ExprResult};
use crate::types::{ExecutableFunction, Expression, Function, NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::any::Any;

use std::sync::Arc;

pub struct NativeFunctionExpr {
    native_function: Arc<dyn NativeFunction>,
}

impl std::fmt::Debug for NativeFunctionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeFunctionExpr")
            .field("name", &self.native_function.name())
            .field("parameters", &self.native_function.parameters())
            .field("return_type", &self.native_function.return_type())
            .finish()
    }
}

impl Clone for NativeFunctionExpr {
    fn clone(&self) -> Self {
        panic!("NativeFunctionExpr cannot be cloned due to boxed trait object")
    }
}

#[async_trait(?Send)]
impl Expression for NativeFunctionExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
        let mut args = Vec::new();

        for param in self.native_function.parameters() {
            let param_name = &param.name;
            if let Some(value) = context.get_variable(param_name) {
                args.push(value.clone());
            } else {
                return Err(format!("Parameter '{}' not found in context", param_name));
            }
        }

        self.native_function.execute(args).await
    }

    fn return_type(&self) -> Type {
        self.native_function.return_type().clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        panic!("NativeFunctionExpr cannot be cloned due to boxed trait object")
    }

    fn documentation(&self) -> Option<&str> {
        self.native_function.documentation()
    }
}

#[async_trait(?Send)]
impl Function for NativeFunctionExpr {
    fn name(&self) -> &str {
        self.native_function.name()
    }

    fn parameters(&self) -> &[Parameter] {
        self.native_function.parameters()
    }

    fn function_return_type(&self) -> &Type {
        self.native_function.return_type()
    }
}

#[async_trait(?Send)]
impl ExecutableFunction for NativeFunctionExpr {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        panic!("NativeFunctionExpr cannot be cloned due to boxed trait object")
    }
}

impl NativeFunctionExpr {
    pub fn new(native_function: Arc<dyn NativeFunction>) -> Self {
        Self { native_function }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompilationUnit;
    use crate::runtime::{Context, Runtime};
    use crate::types::{NativeFunction, Type};
    use std::rc::Rc;

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

    #[async_trait::async_trait(?Send)]
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
            _args: Vec<crate::runtime::ExprResult>,
        ) -> Result<crate::runtime::ExprResult, String> {
            Ok(crate::runtime::ExprResult::String(
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

    #[async_trait::async_trait(?Send)]
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
            _args: Vec<crate::runtime::ExprResult>,
        ) -> Result<crate::runtime::ExprResult, String> {
            Ok(crate::runtime::ExprResult::String(
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

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        let result = expr.evaluate(context).await.unwrap();

        match result {
            crate::runtime::ExprResult::String(s) => assert_eq!(s, "test_result"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(
            expr.documentation(),
            Some("This is a test function with documentation")
        );
    }
}
