use crate::runtime::{Context, ExprResult};
use crate::types::{ExecutableFunction, Expression, Function, NativeFunction, Type};
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

        for (param_name, _) in self.native_function.parameters() {
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
}

#[async_trait(?Send)]
impl Function for NativeFunctionExpr {
    fn name(&self) -> &str {
        self.native_function.name()
    }

    fn parameters(&self) -> &[(String, Type)] {
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
