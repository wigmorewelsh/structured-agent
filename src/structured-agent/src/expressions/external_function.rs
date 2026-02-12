use crate::mcp::McpClient;
use crate::runtime::{Context, ExpressionResult, ExpressionValue};
use crate::types::{ExecutableFunction, Expression, Function, Parameter, Type};
use arrow::array::Array;
use async_trait::async_trait;
use serde_json::json;
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;

pub struct ExternalFunctionExpr {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub mcp_client: Rc<McpClient>,
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
        ExternalFunctionExpr {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            mcp_client: self.mcp_client.clone(),
            documentation: self.documentation.clone(),
        }
    }
}

#[async_trait(?Send)]
impl Expression for ExternalFunctionExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExpressionResult, String> {
        let mut arguments = json!({});

        fn expr_result_to_json(value: &ExpressionValue) -> serde_json::Value {
            match value {
                ExpressionValue::String(s) => json!(s),
                ExpressionValue::Unit => json!(null),
                ExpressionValue::Boolean(b) => json!(b),
                ExpressionValue::List(list) => {
                    if list.len() == 0 {
                        json!([])
                    } else {
                        let values = list.value(0);
                        let mut items = Vec::new();
                        if let Some(string_array) =
                            values.as_any().downcast_ref::<arrow::array::StringArray>()
                        {
                            for i in 0..string_array.len() {
                                items.push(json!(string_array.value(i)));
                            }
                        }
                        json!(items)
                    }
                }
                ExpressionValue::Option(opt) => match opt {
                    Some(inner) => json!({
                        "some": expr_result_to_json(inner)
                    }),
                    None => json!(null),
                },
            }
        }

        for param in &self.parameters {
            let param_name = &param.name;
            if let Some(result) = context.get_variable(param_name) {
                let json_value = expr_result_to_json(&result.value);
                arguments[param_name] = json_value;
            }
        }

        let result = self
            .mcp_client
            .call_tool(&self.name, arguments)
            .await
            .map_err(|e| format!("MCP tool call failed: {}", e))?;

        if result.content.is_empty() {
            Ok(ExpressionResult::new(ExpressionValue::Unit))
        } else {
            if result.content.len() != 1 {
                return Err(format!("Expected one result, got {}", result.content.len()));
            }

            match &*result.content[0] {
                rmcp::model::RawContent::Text(text_content) => Ok(ExpressionResult::new(
                    ExpressionValue::String(text_content.text.clone()),
                )),
                _ => {
                    let content_str = format!("{:?}", result.content);
                    Ok(ExpressionResult::new(ExpressionValue::String(content_str)))
                }
            }
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

    fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }

    fn name(&self) -> Option<&str> {
        Some(self.name.as_str())
    }
}

impl ExternalFunctionExpr {
    pub fn new(
        name: String,
        parameters: Vec<Parameter>,
        return_type: Type,
        mcp_client: Rc<McpClient>,
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

#[async_trait(?Send)]
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Type;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_external_function_documentation() {
        let client = Rc::new(McpClient::new_stdio("echo", vec![]).await.unwrap());

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

#[async_trait(?Send)]
impl ExecutableFunction for ExternalFunctionExpr {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        Box::new(self.clone())
    }
}
