use crate::types::{Context, ExprResult, Expression, Type};
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

impl Expression for CallExpr {
    fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let function_name = if self.is_method || self.target.is_empty() {
            self.function.clone()
        } else {
            format!("{}::{}", self.target, self.function)
        };

        let function_info = context
            .registry
            .get_function(&function_name)
            .ok_or_else(|| {
                if self.is_method {
                    format!("Unknown method: {}.{}", self.target, self.function)
                } else {
                    format!("Unknown function: {}", function_name)
                }
            })?
            .clone();

        if self.arguments.len() != function_info.parameters.len() {
            return Err(format!(
                "Function {} expects {} arguments, got {}",
                function_name,
                function_info.parameters.len(),
                self.arguments.len()
            ));
        }

        let mut args = Vec::new();
        for arg in &self.arguments {
            args.push(arg.evaluate(context)?);
        }

        let mut function_context = context.create_child();

        for (i, (param_name, _param_type)) in function_info.parameters.iter().enumerate() {
            function_context.set_variable(param_name.clone(), args[i].clone());
        }

        let _result = function_info.evaluate(&mut function_context)?;

        let llm_result = context.engine.untyped(&function_context);

        Ok(ExprResult::String(llm_result))
    }

    fn return_type(&self) -> Type {
        let function_name = if self.is_method || self.target.is_empty() {
            self.function.clone()
        } else {
            format!("{}::{}", self.target, self.function)
        };

        let context = Context::new();
        if let Some(function) = context.registry.get_function(&function_name) {
            function.return_type.clone()
        } else {
            Type::unit()
        }
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

    #[test]
    fn test_unknown_method() {
        let expr = CallExpr {
            target: "obj".to_string(),
            function: "unknown_method".to_string(),
            arguments: vec![],
            is_method: true,
        };

        let mut context = Context::new();
        let result = expr.evaluate(&mut context);

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

    #[test]
    fn test_call_with_registry() {
        let mut context = Context::new();

        let function_info = FunctionExpr {
            name: "hello".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            body: vec![Box::new(StringLiteralExpr {
                value: "Hello, World!".to_string(),
            })],
        };
        context.registry.register_function(function_info);

        let expr = CallExpr {
            target: String::new(),
            function: "hello".to_string(),
            arguments: vec![],
            is_method: false,
        };

        let result = expr.evaluate(&mut context).unwrap();
        match result {
            ExprResult::String(s) => assert_eq!(s, "Hello, World!"),
            _ => panic!("Expected string result"),
        }
    }

    #[test]
    fn test_unknown_static_function() {
        let expr = CallExpr {
            target: "Unknown".to_string(),
            function: "func".to_string(),
            arguments: vec![],
            is_method: false,
        };

        let mut context = Context::new();
        let result = expr.evaluate(&mut context);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Unknown function: Unknown::func")
        );
    }
}
