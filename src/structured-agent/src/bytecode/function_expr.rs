use crate::bytecode::{CompiledFunction, VM};
use crate::runtime::{Context, ExpressionResult};
use crate::types::{ExecutableFunction, Expression, Function, Parameter, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

pub struct BytecodeFunctionExpr {
    compiled: Arc<CompiledFunction>,
}

impl BytecodeFunctionExpr {
    pub fn new(compiled: CompiledFunction) -> Self {
        Self {
            compiled: Arc::new(compiled),
        }
    }
}

impl std::fmt::Debug for BytecodeFunctionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BytecodeFunctionExpr")
            .field("name", &self.compiled.name)
            .field("parameters", &self.compiled.parameters)
            .field("return_type", &self.compiled.return_type)
            .field(
                "instructions",
                &format!("[{} instructions]", self.compiled.instructions.len()),
            )
            .finish()
    }
}

impl Clone for BytecodeFunctionExpr {
    fn clone(&self) -> Self {
        BytecodeFunctionExpr {
            compiled: Arc::clone(&self.compiled),
        }
    }
}

#[async_trait(?Send)]
impl Expression for BytecodeFunctionExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let vm = VM::new(context.runtime_rc());
        vm.execute(&self.compiled, context).await
    }

    fn return_type(&self) -> Type {
        self.compiled.return_type.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(self.clone())
    }

    fn documentation(&self) -> Option<&str> {
        None
    }

    fn name(&self) -> Option<&str> {
        Some(self.compiled.name.as_str())
    }
}

#[async_trait(?Send)]
impl Function for BytecodeFunctionExpr {
    fn name(&self) -> &str {
        &self.compiled.name
    }

    fn parameters(&self) -> &[Parameter] {
        &self.compiled.parameters
    }

    fn function_return_type(&self) -> &Type {
        &self.compiled.return_type
    }
}

#[async_trait(?Send)]
impl ExecutableFunction for BytecodeFunctionExpr {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        Box::new(self.clone())
    }
}
