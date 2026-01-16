use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use std::any::Any;

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

impl Expression for FunctionExpr {
    fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let mut last_result = ExprResult::Unit;
        for statement in &self.body {
            last_result = statement.evaluate(context)?;
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
