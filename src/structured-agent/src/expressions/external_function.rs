use crate::runtime::{Context, ExprResult};
use crate::types::{ExecutableFunction, Expression, Function, Type};
use async_trait::async_trait;
use serde_json::json;
use std::any::Any;

pub struct ExternalFunctionExpr {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub return_type: Type,
    pub client_index: usize,
}

impl std::fmt::Debug for ExternalFunctionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalFunctionExpr")
            .field("name", &self.name)
            .field("parameters", &self.parameters)
            .field("return_type", &self.return_type)
            .field("client_index", &self.client_index)
            .finish()
    }
}

impl Clone for ExternalFunctionExpr {
    fn clone(&self) -> Self {
        ExternalFunctionExpr {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            client_index: self.client_index,
        }
    }
}

#[async_trait(?Send)]
impl Expression for ExternalFunctionExpr {
    async fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let runtime = context.runtime();
        let mut runtime_mut = runtime.clone();

        let client = runtime_mut
            .get_mcp_client_for_function(&self.name)
            .ok_or_else(|| format!("No MCP client found for function: {}", self.name))?;

        let mut arguments = json!({});

        for (param_name, _param_type) in &self.parameters {
            if let Some(value) = context.get_variable(param_name) {
                let json_value = match value {
                    ExprResult::String(s) => json!(s),
                    ExprResult::Unit => json!(null),
                };
                arguments[param_name] = json_value;
            }
        }

        let result = client
            .call_tool(&self.name, arguments)
            .await
            .map_err(|e| format!("MCP tool call failed: {}", e))?;

        if result.content.is_empty() {
            Ok(ExprResult::Unit)
        } else {
            let content_str = result
                .content
                .iter()
                .map(|c| format!("{:?}", c))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(ExprResult::String(content_str))
        }
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

impl ExternalFunctionExpr {
    pub fn new(
        name: String,
        parameters: Vec<(String, Type)>,
        return_type: Type,
        client_index: usize,
    ) -> Self {
        Self {
            name,
            parameters,
            return_type,
            client_index,
        }
    }
}

#[async_trait(?Send)]
impl Function for ExternalFunctionExpr {
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
impl ExecutableFunction for ExternalFunctionExpr {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        Box::new(self.clone())
    }
}
