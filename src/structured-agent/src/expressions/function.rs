use crate::runtime::{Context, ExprResult};
use crate::types::{ExecutableFunction, Expression, Function, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

pub struct FunctionExpr {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub return_type: Type,
    pub body: Vec<Box<dyn Expression>>,
}

impl std::fmt::Debug for FunctionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionExpr")
            .field("name", &self.name)
            .field("parameters", &self.parameters)
            .field("return_type", &self.return_type)
            .field("body", &format!("[{} statements]", self.body.len()))
            .finish()
    }
}

impl Clone for FunctionExpr {
    fn clone(&self) -> Self {
        FunctionExpr {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            body: self.body.iter().map(|expr| expr.clone_box()).collect(),
        }
    }
}

#[async_trait(?Send)]
impl Expression for FunctionExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
        let mut last_result = ExprResult::Unit;
        for statement in &self.body {
            last_result = statement.evaluate(context.clone()).await?;
        }

        if !context.events.borrow().is_empty() {
            let engine_response = context.runtime().engine().untyped(&context).await;
            return Ok(ExprResult::String(engine_response));
        }

        Ok(last_result)
    }

    fn return_type(&self) -> Type {
        self.return_type.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(self.clone())
    }
}

#[async_trait(?Send)]
impl Function for FunctionExpr {
    fn name(&self) -> &str {
        &self.name
    }

    fn parameters(&self) -> &[(String, Type)] {
        &self.parameters
    }

    fn function_return_type(&self) -> &Type {
        &self.return_type
    }
}

#[async_trait(?Send)]
impl ExecutableFunction for FunctionExpr {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        Box::new(self.clone())
    }
}
