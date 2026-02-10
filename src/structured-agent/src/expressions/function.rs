use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{ExecutableFunction, Expression, Function, Parameter, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

pub struct FunctionExpr {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub body: Vec<Box<dyn Expression>>,
    pub documentation: Option<String>,
}

impl std::fmt::Debug for FunctionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionExpr")
            .field("name", &self.name)
            .field("parameters", &self.parameters)
            .field("return_type", &self.return_type)
            .field("body", &format!("[{} statements]", self.body.len()))
            .field("documentation", &self.documentation)
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
            documentation: self.documentation.clone(),
        }
    }
}

#[async_trait(?Send)]
impl Expression for FunctionExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let mut last_result = ExpressionValue::Unit;
        for statement in &self.body {
            let result = statement.evaluate(context.clone()).await?;
            last_result = result.value;

            if context.has_return_value() {
                return Ok(context.get_return_value().unwrap());
            }
        }

        if context.has_events() {
            if matches!(self.return_type, Type::Unit) {
                return Ok(ExpressionResult::new(ExpressionValue::Unit));
            } else {
                let engine_response = context
                    .runtime()
                    .engine()
                    .typed(&context, &self.return_type)
                    .await?;
                return Ok(ExpressionResult::new(engine_response));
            }
        }

        Ok(ExpressionResult::new(last_result))
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

    fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }

    fn name(&self) -> Option<&str> {
        Some(self.name.as_str())
    }
}

#[async_trait(?Send)]
impl Function for FunctionExpr {
    fn name(&self) -> &str {
        &self.name
    }

    fn parameters(&self) -> &[Parameter] {
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
