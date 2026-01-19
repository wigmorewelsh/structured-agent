use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;

pub struct CallExpr {
    pub target: String,
    pub function: String,
    pub arguments: Vec<Box<dyn Expression>>,
    pub is_method: bool,
}

impl std::fmt::Debug for CallExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallExpr")
            .field("target", &self.target)
            .field("function", &self.function)
            .field("arguments", &format!("[{} args]", self.arguments.len()))
            .field("is_method", &self.is_method)
            .finish()
    }
}

#[async_trait(?Send)]
impl Expression for CallExpr {
    async fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let function_name = if self.is_method || self.target.is_empty() {
            self.function.clone()
        } else {
            format!("{}::{}", self.target, self.function)
        };

        let function_info = context
            .runtime()
            .get_function(&function_name)
            .ok_or_else(|| {
                if self.is_method {
                    format!("Unknown method: {}.{}", self.target, self.function)
                } else {
                    format!("Unknown function: {}", function_name)
                }
            })?;

        let parameters = function_info.parameters().to_vec();

        if self.arguments.len() != parameters.len() {
            return Err(format!(
                "Function {} expects {} arguments, got {}",
                function_name,
                parameters.len(),
                self.arguments.len()
            ));
        }

        let mut args = Vec::new();
        for arg in &self.arguments {
            args.push(arg.evaluate(context).await?);
        }

        let mut function_context = context.create_child();

        for (i, (param_name, _param_type)) in parameters.iter().enumerate() {
            function_context.set_variable(param_name.clone(), args[i].clone());
        }

        let function_info = context.runtime().get_function(&function_name).unwrap();

        let result = function_info.evaluate(&mut function_context).await?;

        Ok(result)
    }

    fn return_type(&self) -> Type {
        let _function_name = if self.is_method || self.target.is_empty() {
            self.function.clone()
        } else {
            format!("{}::{}", self.target, self.function)
        };

        Type::unit()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(CallExpr {
            target: self.target.clone(),
            function: self.function.clone(),
            arguments: self.arguments.iter().map(|arg| arg.clone_box()).collect(),
            is_method: self.is_method,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::{FunctionExpr, StringLiteralExpr};
    use crate::runtime::Runtime;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_unknown_method() {
        let expr = CallExpr {
            target: "obj".to_string(),
            function: "unknown_method".to_string(),
            arguments: vec![],
            is_method: true,
        };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = expr.evaluate(&mut context).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Unknown method: obj.unknown_method")
        );
    }

    #[test]
    fn test_call_clone() {
        let expr = CallExpr {
            target: "obj".to_string(),
            function: "method".to_string(),
            arguments: vec![],
            is_method: true,
        };

        let cloned = expr.clone_box();

        // Test that cloning produces equivalent objects
        assert_eq!(
            expr.target,
            cloned.as_any().downcast_ref::<CallExpr>().unwrap().target
        );
        assert_eq!(
            expr.function,
            cloned.as_any().downcast_ref::<CallExpr>().unwrap().function
        );
    }

    #[tokio::test]
    async fn test_call_with_registry() {
        let mut runtime = Runtime::new();

        let function_info = FunctionExpr {
            name: "hello".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            body: vec![Box::new(StringLiteralExpr {
                value: "Hello, World!".to_string(),
            })],
        };
        runtime.register_function(function_info);

        let runtime = Rc::new(runtime);
        let mut context = Context::with_runtime(runtime);

        let expr = CallExpr {
            target: String::new(),
            function: "hello".to_string(),
            arguments: vec![],
            is_method: false,
        };

        let result = expr.evaluate(&mut context).await.unwrap();
        match result {
            ExprResult::String(s) => assert_eq!(s, "Hello, World!"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_unknown_static_function() {
        let expr = CallExpr {
            target: "Unknown".to_string(),
            function: "func".to_string(),
            arguments: vec![],
            is_method: false,
        };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = expr.evaluate(&mut context).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Unknown function: Unknown::func")
        );
    }
}
