use crate::mcp::McpClient;
use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{ExecutableFunction, Function, Parameter, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

pub struct ExternalFunctionExpr {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub mcp_client: Arc<McpClient>,
    pub documentation: Option<String>,
}

impl std::fmt::Debug for ExternalFunctionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalFunctionExpr")
            .field("name", &self.name)
            .field("parameters", &self.parameters)
            .field("return_type", &self.return_type)
            .field("mcp_client", &"McpClient")
            .field("documentation", &self.documentation)
            .finish()
    }
}

impl Clone for ExternalFunctionExpr {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            mcp_client: self.mcp_client.clone(),
            documentation: self.documentation.clone(),
        }
    }
}

impl ExternalFunctionExpr {
    pub fn new(
        name: String,
        parameters: Vec<Parameter>,
        return_type: Type,
        mcp_client: Arc<McpClient>,
        documentation: Option<String>,
    ) -> Self {
        Self {
            name,
            parameters,
            return_type,
            mcp_client,
            documentation,
        }
    }
}

#[async_trait]
impl Function for ExternalFunctionExpr {
    fn name(&self) -> &str {
        &self.name
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn function_return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(
        &self,
        context: Context,
        args: Vec<ExpressionResult>,
    ) -> Result<(Context, ExpressionResult), String> {
        let named_args: Vec<(String, ExpressionValue)> = self
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name.clone(), args[i].value.clone()))
            .collect();

        let result = self
            .mcp_client
            .call_tool(&self.name, &named_args, &self.return_type)
            .await
            .map_err(|e| format!("MCP tool call failed: {}", e));

        match result {
            Err(e) => Err(e),
            Ok(value) => Ok((context, ExpressionResult::new(value))),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Function> {
        Box::new(self.clone())
    }

    fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Type;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_external_function_documentation() {
        let client = Arc::new(McpClient::new_stdio("echo", vec![], None).await.unwrap());

        let expr_with_docs = ExternalFunctionExpr {
            name: "test_function".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            mcp_client: client.clone(),
            documentation: Some("This is a test external function".to_string()),
        };

        assert_eq!(
            expr_with_docs.documentation(),
            Some("This is a test external function")
        );

        let expr_without_docs = ExternalFunctionExpr {
            name: "undocumented_function".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            mcp_client: client,
            documentation: None,
        };

        assert_eq!(expr_without_docs.documentation(), None);
    }
}

#[async_trait]
impl ExecutableFunction for ExternalFunctionExpr {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        Box::new(self.clone())
    }
}
