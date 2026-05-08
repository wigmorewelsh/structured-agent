use crate::expressions::ExternalFunctionExpr;
use crate::runtime::{ExpressionValue, RuntimeError};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, FunctionProvider, Parameter, Type,
};
use arrow::array::Array;
use async_trait::async_trait;
use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::{RoleClient, ServiceError, ServiceExt};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

type RmcpClient = rmcp::service::RunningService<RoleClient, ()>;

#[derive(Debug)]
pub enum McpError {
    ConnectionError(String),
    ProtocolError(String),
    ToolError(String),
    SdkError(String),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            McpError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            McpError::ToolError(msg) => write!(f, "Tool error: {}", msg),
            McpError::SdkError(msg) => write!(f, "SDK error: {}", msg),
        }
    }
}

impl Error for McpError {}

impl From<ServiceError> for McpError {
    fn from(e: ServiceError) -> Self {
        McpError::SdkError(e.to_string())
    }
}

impl From<std::io::Error> for McpError {
    fn from(e: std::io::Error) -> Self {
        McpError::ConnectionError(e.to_string())
    }
}

fn serialise_args(
    args: &[(String, ExpressionValue)],
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for (name, value) in args {
        let json_value = if let Ok(s) = value.as_string() {
            Value::String(s)
        } else if let Ok(b) = value.as_boolean() {
            Value::Bool(b)
        } else if let Ok(n) = value.as_integer() {
            Value::Number(n.into())
        } else if value.type_name() == "Unit" {
            Value::Null
        } else if let Ok(list) = value.as_list() {
            if list.len() == 0 {
                Value::Array(vec![])
            } else {
                let values = list.value(0);
                let mut items = Vec::new();
                if let Some(string_array) =
                    values.as_any().downcast_ref::<arrow::array::StringArray>()
                {
                    for i in 0..string_array.len() {
                        items.push(Value::String(string_array.value(i).to_string()));
                    }
                }
                Value::Array(items)
            }
        } else {
            Value::Null
        };
        map.insert(name.clone(), json_value);
    }
    map
}

fn json_schema_to_type(schema: &Value) -> Option<Type> {
    match schema.get("type").and_then(|t| t.as_str()) {
        Some("integer") => Some(Type::int()),
        Some("boolean") => Some(Type::boolean()),
        Some("string") => Some(Type::string()),
        Some("array") => {
            let item_type = schema
                .get("items")
                .and_then(|items| json_schema_to_type(items))
                .unwrap_or_else(Type::string);
            Some(Type::list(item_type))
        }
        _ => None,
    }
}

fn parse_text_content(text: &str, return_type: &Type) -> Result<ExpressionValue, McpError> {
    if return_type.is_int() {
        text.trim()
            .parse::<i64>()
            .map(ExpressionValue::integer)
            .map_err(|_| McpError::ToolError(format!("Cannot parse {:?} as integer", text)))
    } else if return_type.is_boolean() {
        text.trim()
            .parse::<bool>()
            .map(ExpressionValue::boolean)
            .map_err(|_| McpError::ToolError(format!("Cannot parse {:?} as boolean", text)))
    } else if return_type.is_list() {
        let json: serde_json::Value = serde_json::from_str(text)
            .map_err(|_| McpError::ToolError(format!("Cannot parse {:?} as JSON", text)))?;
        let arr = json
            .as_array()
            .ok_or_else(|| McpError::ToolError(format!("Expected JSON array, got {:?}", text)))?;
        let elements: Vec<ExpressionValue> = arr
            .iter()
            .map(|v| ExpressionValue::string(v.as_str().unwrap_or("").to_string()))
            .collect();
        ExpressionValue::from_elements(elements).map_err(McpError::ToolError)
    } else {
        Ok(ExpressionValue::string(text.to_string()))
    }
}

pub struct McpClient {
    client: Arc<RwLock<Option<RmcpClient>>>,
    command: String,
    args: Vec<String>,
    working_dir: Option<String>,
}

impl McpClient {
    pub async fn new_stdio(
        command: &str,
        args: Vec<String>,
        working_dir: Option<String>,
    ) -> std::result::Result<Self, McpError> {
        Ok(Self {
            client: Arc::new(RwLock::new(None)),
            command: command.to_string(),
            args,
            working_dir,
        })
    }

    async fn ensure_connected(&self) -> std::result::Result<(), McpError> {
        let client_lock = self.client.read().await;
        if client_lock.is_none() {
            drop(client_lock);
            self.connect().await?;
        }
        Ok(())
    }

    async fn connect(&self) -> std::result::Result<(), McpError> {
        use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
        use tokio::process::Command;

        let transport = TokioChildProcess::new(Command::new(&self.command).configure(|cmd| {
            for arg in &self.args {
                cmd.arg(arg);
            }
            if let Some(ref dir) = self.working_dir {
                cmd.current_dir(dir);
            }
        }))?;

        let service = ()
            .serve(transport)
            .await
            .map_err(|e| McpError::ConnectionError(format!("Failed to start client: {}", e)))?;

        let mut client_lock = self.client.write().await;
        *client_lock = Some(service);

        Ok(())
    }

    pub async fn list_tools(&self) -> std::result::Result<Vec<Tool>, McpError> {
        self.ensure_connected().await?;

        let client_lock = self.client.read().await;
        let client = client_lock
            .as_ref()
            .ok_or_else(|| McpError::ConnectionError("No client available".to_string()))?;

        let tools = client
            .list_all_tools()
            .await
            .map_err(|e| McpError::ProtocolError(format!("Failed to list tools: {}", e)))?;

        Ok(tools)
    }

    pub async fn call_tool(
        &self,
        name: &str,
        args: &[(String, ExpressionValue)],
        return_type: &Type,
    ) -> Result<ExpressionValue, McpError> {
        self.ensure_connected().await?;

        let client_lock = self.client.read().await;
        let client = client_lock
            .as_ref()
            .ok_or_else(|| McpError::ConnectionError("No client available".to_string()))?;

        let params = serialise_args(args);

        let request = CallToolRequestParams {
            name: name.to_string().into(),
            arguments: Some(params),
            meta: None,
            task: None,
        };

        let response = client
            .call_tool(request)
            .await
            .map_err(|e| McpError::ToolError(format!("Failed to call tool: {}", e)))?;

        if response.content.is_empty() {
            return Ok(ExpressionValue::unit());
        }

        if response.content.len() != 1 {
            return Err(McpError::ToolError(format!(
                "Expected one result, got {}",
                response.content.len()
            )));
        }

        match &*response.content[0] {
            rmcp::model::RawContent::Text(text_content) => {
                parse_text_content(&text_content.text, return_type)
            }
            other => Ok(ExpressionValue::string(format!("{:?}", other))),
        }
    }

    pub async fn shutdown(&self) -> std::result::Result<(), McpError> {
        let mut client_lock = self.client.write().await;
        if let Some(client) = client_lock.take() {
            client
                .cancel()
                .await
                .map_err(|e| McpError::ConnectionError(format!("Failed to shutdown: {}", e)))?;
        }
        Ok(())
    }
}

#[async_trait]
impl FunctionProvider for McpClient {
    async fn list_functions(&self) -> Result<Vec<ExternalFunctionDefinition>, RuntimeError> {
        let tools = self.list_tools().await.map_err(|e| {
            RuntimeError::ExecutionError(format!("Failed to list MCP tools: {}", e))
        })?;

        let definitions = tools
            .into_iter()
            .map(|tool| {
                let parameters = tool
                    .input_schema
                    .get("properties")
                    .and_then(|properties| properties.as_object())
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| {
                                Parameter::new(
                                    k.clone(),
                                    json_schema_to_type(v).unwrap_or_else(Type::string),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let return_type = tool
                    .output_schema
                    .as_ref()
                    .and_then(|schema| json_schema_to_type(&Value::Object((**schema).clone())))
                    .unwrap_or_else(|| Type::Generic("_".to_string()));

                ExternalFunctionDefinition::new_with_docs(
                    tool.name.to_string(),
                    parameters,
                    return_type,
                    tool.description.map(|d| d.to_string()),
                )
            })
            .collect();

        Ok(definitions)
    }

    async fn create_expression(
        &self,
        definition: &ExternalFunctionDefinition,
    ) -> Result<Arc<dyn ExecutableFunction>, RuntimeError> {
        let expr = ExternalFunctionExpr::new(
            definition.name.clone(),
            definition.parameters.clone(),
            definition.return_type.clone(),
            Arc::new(self.clone()),
            definition.documentation.clone(),
        );
        Ok(Arc::new(expr))
    }
}

impl Clone for McpClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            working_dir: self.working_dir.clone(),
        }
    }
}

pub fn create_client_info(name: &str, version: &str) -> rmcp::model::Implementation {
    rmcp::model::Implementation {
        name: name.into(),
        version: version.into(),
        title: None,
        description: None,
        icons: None,
        website_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ExpressionValue;
    use serde_json::json;

    #[test]
    fn test_create_client_info() {
        let client_info = create_client_info("test-agent", "0.1.0");
        assert_eq!(client_info.name, "test-agent");
        assert_eq!(client_info.version, "0.1.0");
    }

    #[tokio::test]
    async fn test_mcp_client_creation() {
        let result = McpClient::new_stdio("echo", vec![], None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_tools_with_invalid_server() {
        let client = McpClient::new_stdio("echo", vec![], None).await.unwrap();
        let result = client.list_tools().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_tool_with_invalid_server() {
        let client = McpClient::new_stdio("echo", vec![], None).await.unwrap();
        let result = client
            .call_tool(
                "test_tool",
                &[("arg".to_string(), ExpressionValue::string("value"))],
                &Type::string(),
            )
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn call_tool_serialises_string_arg() {
        let result = serialise_args(&[("msg".to_string(), ExpressionValue::string("hello"))]);
        assert_eq!(serde_json::Value::Object(result), json!({"msg": "hello"}));
    }

    #[test]
    fn call_tool_serialises_bool_arg() {
        let result = serialise_args(&[("flag".to_string(), ExpressionValue::boolean(true))]);
        assert_eq!(serde_json::Value::Object(result), json!({"flag": true}));
    }

    #[test]
    fn call_tool_serialises_integer_arg() {
        let result = serialise_args(&[("n".to_string(), ExpressionValue::integer(42))]);
        assert_eq!(serde_json::Value::Object(result), json!({"n": 42}));
    }

    #[test]
    fn call_tool_serialises_list_of_strings_arg() {
        let list = ExpressionValue::from_elements(vec![
            ExpressionValue::string("a"),
            ExpressionValue::string("b"),
        ])
        .unwrap();
        let result = serialise_args(&[("items".to_string(), list)]);
        assert_eq!(
            serde_json::Value::Object(result),
            json!({"items": ["a", "b"]})
        );
    }

    #[test]
    fn parse_text_content_string() {
        let result = parse_text_content("hello", &Type::string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_string().unwrap(), "hello");
    }

    #[test]
    fn parse_text_content_integer() {
        let result = parse_text_content("42", &Type::int());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_integer().unwrap(), 42);
    }

    #[test]
    fn parse_text_content_integer_invalid() {
        let result = parse_text_content("abc", &Type::int());
        assert!(result.is_err());
    }

    #[test]
    fn parse_text_content_bool_true() {
        let result = parse_text_content("true", &Type::boolean());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_boolean().unwrap(), true);
    }

    #[test]
    fn parse_text_content_bool_false() {
        let result = parse_text_content("false", &Type::boolean());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_boolean().unwrap(), false);
    }

    #[test]
    fn parse_text_content_bool_invalid() {
        let result = parse_text_content("yes", &Type::boolean());
        assert!(result.is_err());
    }

    #[test]
    fn parse_text_content_list_of_strings() {
        let result = parse_text_content("[\"a\",\"b\",\"c\"]", &Type::list(Type::string()));
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
        let value = result.unwrap();
        let list = value.as_list().unwrap();
        assert_eq!(list.value(0).len(), 3);
    }

    #[test]
    fn parse_text_content_empty_list() {
        let result = parse_text_content("[]", &Type::list(Type::string()));
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
        let value = result.unwrap();
        let list = value.as_list().unwrap();
        assert_eq!(list.value(0).len(), 0);
    }

    #[test]
    fn parse_text_content_list_invalid_json() {
        let result = parse_text_content("not json", &Type::list(Type::string()));
        assert!(result.is_err());
    }

    #[test]
    fn parse_text_content_list_not_array() {
        let result = parse_text_content("{\"key\":\"val\"}", &Type::list(Type::string()));
        assert!(result.is_err());
    }

    #[test]
    fn call_tool_serialises_unit_arg_as_null() {
        let result = serialise_args(&[("x".to_string(), ExpressionValue::unit())]);
        assert_eq!(serde_json::Value::Object(result), json!({"x": null}));
    }
}
