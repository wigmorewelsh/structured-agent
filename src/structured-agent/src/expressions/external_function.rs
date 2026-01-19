use crate::mcp::McpClient;
use crate::runtime::{Context, ExprResult};
use crate::types::{ExecutableFunction, Expression, Function, Type};
use async_trait::async_trait;
use serde_json::json;
use std::any::Any;
use std::rc::Rc;

pub struct ExternalFunctionExpr {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub return_type: Type,
    pub mcp_client: Rc<McpClient>,
}

impl std::fmt::Debug for ExternalFunctionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalFunctionExpr")
            .field("name", &self.name)
            .field("parameters", &self.parameters)
            .field("return_type", &self.return_type)
            .field("mcp_client", &"McpClient")
            .finish()
    }
}

impl Clone for ExternalFunctionExpr {
    fn clone(&self) -> Self {
        ExternalFunctionExpr {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            mcp_client: self.mcp_client.clone(),
        }
    }
}

#[async_trait(?Send)]
impl Expression for ExternalFunctionExpr {
    async fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        let mut arguments = json!({});

        // Map parameter names to values from the context
        for (param_name, _param_type) in &self.parameters {
            if let Some(value) = context.get_variable(param_name) {
                let json_value = match value {
                    ExprResult::String(s) => json!(s),
                    ExprResult::Unit => json!(null),
                };
                arguments[param_name] = json_value;
            }
        }

        let result = self
            .mcp_client
            .call_tool(&self.name, arguments)
            .await
            .map_err(|e| format!("MCP tool call failed: {}", e))?;

        if result.content.is_empty() {
            Ok(ExprResult::Unit)
        } else {
            // For now, just convert the entire result to a string
            let content_str = format!("{:?}", result.content);
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
        mcp_client: Rc<McpClient>,
    ) -> Self {
        Self {
            name,
            parameters,
            return_type,
            mcp_client,
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
