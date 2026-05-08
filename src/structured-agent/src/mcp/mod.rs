use crate::expressions::ExternalFunctionExpr;
use crate::runtime::{ExpressionValue, RuntimeError};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, FunctionProvider, Parameter, Type,
};
use arrow::array::Array;
use arrow::datatypes::{DataType, Field};
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
                } else if let Some(int_array) =
                    values.as_any().downcast_ref::<arrow::array::Int64Array>()
                {
                    for i in 0..int_array.len() {
                        items.push(Value::Number(int_array.value(i).into()));
                    }
                } else if let Some(bool_array) =
                    values.as_any().downcast_ref::<arrow::array::BooleanArray>()
                {
                    for i in 0..bool_array.len() {
                        items.push(Value::Bool(bool_array.value(i)));
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

fn ty_to_datatype(ty: &Type) -> DataType {
    if ty.is_string() {
        DataType::Utf8
    } else if ty.is_int() {
        DataType::Int64
    } else if ty.is_boolean() {
        DataType::Boolean
    } else if ty.is_unit() {
        DataType::Null
    } else if ty.is_list() {
        if let Type::Parameterized(_, args) = ty {
            if let Some(inner) = args.first() {
                return DataType::List(Arc::new(Field::new("item", ty_to_datatype(inner), true)));
            }
        }
        DataType::List(Arc::new(Field::new("item", DataType::Utf8, true)))
    } else {
        DataType::Null
    }
}



fn json_to_expression(json: &serde_json::Value, ty: &Type) -> Result<ExpressionValue, McpError> {
    if ty.is_string() {
        return Ok(ExpressionValue::string(
            json.as_str().unwrap_or(&json.to_string()).to_string(),
        ));
    }
    if ty.is_int() {
        let n = json.as_i64()
            .or_else(|| json.as_str().and_then(|s| s.parse::<i64>().ok()))
            .ok_or_else(|| McpError::ToolError(format!("Cannot parse {:?} as integer", json)))?;
        return Ok(ExpressionValue::integer(n));
    }
    if ty.is_boolean() {
        let b = json.as_bool()
            .or_else(|| json.as_str().and_then(|s| s.parse::<bool>().ok()))
            .ok_or_else(|| McpError::ToolError(format!("Cannot parse {:?} as boolean", json)))?;
        return Ok(ExpressionValue::boolean(b));
    }
    if ty.is_unit() {
        return Ok(ExpressionValue::unit());
    }
    if ty.is_list() {
        if let Type::Parameterized(_, args) = ty {
            if let Some(inner) = args.first() {
                let arr = json.as_array()
                    .ok_or_else(|| McpError::ToolError(format!("Expected JSON array, got {:?}", json)))?;
                let elements: Result<Vec<ExpressionValue>, McpError> =
                    arr.iter().map(|v| json_to_expression(v, inner)).collect();
                return ExpressionValue::from_elements(elements?).map_err(McpError::ToolError);
            }
        }
    }
    if ty.is_option() {
        if let Type::Parameterized(_, args) = ty {
            if let Some(inner) = args.first() {
                if json.is_null() {
                    return Ok(ExpressionValue::option_none_with_type(ty_to_datatype(inner)));
                }
                return json_to_expression(json, inner).map(ExpressionValue::option_some);
            }
        }
    }
    Err(McpError::ToolError(format!(
        "Type '{}' must be explicitly declared in SA; user-defined types are not supported for external function calls",
        ty.name()
    )))
}

fn parse_text_content(text: &str, return_type: &Type) -> Result<ExpressionValue, McpError> {
    if return_type.is_string() {
        return Ok(ExpressionValue::string(text.to_string()));
    }
    let json: serde_json::Value = serde_json::from_str(text)
        .map_err(|_| McpError::ToolError(format!("Cannot parse {:?} as JSON", text)))?;
    json_to_expression(&json, return_type)
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

        if response.is_error == Some(true) {
            let msg = response.content.first()
                .and_then(|block| match &**block {
                    rmcp::model::RawContent::Text(t) => Some(t.text.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "Tool returned an error".to_string());
            return Err(McpError::ToolError(msg));
        }

        if response.content.is_empty() {
            return Ok(ExpressionValue::unit());
        }

        if response.content.len() > 1 {
            let mut elements = Vec::new();
            for block in &response.content {
                match &**block {
                    rmcp::model::RawContent::Text(text_content) => {
                        elements.push(ExpressionValue::string(text_content.text.clone()));
                    }
                    _ => {
                        return Err(McpError::ToolError(
                            "Multi-block response contained non-text content".to_string(),
                        ));
                    }
                }
            }
            return ExpressionValue::from_elements(elements).map_err(McpError::ToolError);
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
    fn call_tool_serialises_list_of_integers_arg() {
        let list = ExpressionValue::from_elements(vec![
            ExpressionValue::integer(1),
            ExpressionValue::integer(2),
            ExpressionValue::integer(3),
        ])
        .unwrap();
        let result = serialise_args(&[("items".to_string(), list)]);
        assert_eq!(
            serde_json::Value::Object(result),
            json!({"items": [1, 2, 3]})
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

    #[test]
    fn json_to_expression_string() {
        let result = json_to_expression(&serde_json::json!("hi"), &Type::string()).unwrap();
        assert_eq!(result.as_string().unwrap(), "hi");
    }

    #[test]
    fn json_to_expression_int() {
        let result = json_to_expression(&serde_json::json!(42), &Type::int()).unwrap();
        assert_eq!(result.as_integer().unwrap(), 42);
    }

    #[test]
    fn json_to_expression_bool() {
        let result = json_to_expression(&serde_json::json!(true), &Type::boolean()).unwrap();
        assert!(result.as_boolean().unwrap());
    }

    #[test]
    fn json_to_expression_list_of_int() {
        let result = json_to_expression(&serde_json::json!([1, 2, 3]), &Type::list(Type::int())).unwrap();
        let elements = result.as_list_elements().unwrap();
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].as_integer().unwrap(), 1);
        assert_eq!(elements[1].as_integer().unwrap(), 2);
        assert_eq!(elements[2].as_integer().unwrap(), 3);
    }

    #[test]
    fn json_to_expression_list_of_bool() {
        let result = json_to_expression(&serde_json::json!([true, false]), &Type::list(Type::boolean())).unwrap();
        let elements = result.as_list_elements().unwrap();
        assert_eq!(elements.len(), 2);
        assert!(elements[0].as_boolean().unwrap());
        assert!(!elements[1].as_boolean().unwrap());
    }

    #[test]
    fn json_to_expression_option_some() {
        let result = json_to_expression(&serde_json::json!("x"), &Type::option(Type::string())).unwrap();
        let inner = result.as_option().unwrap().unwrap();
        assert_eq!(inner.as_string().unwrap(), "x");
    }

    #[test]
    fn json_to_expression_option_none() {
        let result = json_to_expression(&serde_json::Value::Null, &Type::option(Type::string())).unwrap();
        assert!(result.as_option().unwrap().is_none());
    }

    #[test]
    fn json_to_expression_unknown_type_is_error() {
        use nonempty::NonEmpty;
        use structured_agent_runtime::symbols::DefinitionPath;
        let ty = Type::Named(DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new("main".to_string())),
            "Person",
        ));
        let result = json_to_expression(&serde_json::json!({"name": "alice"}), &ty);
        assert!(result.is_err());
    }

    #[test]
    fn parse_text_content_list_of_int() {
        let result = parse_text_content("[1,2,3]", &Type::list(Type::int())).unwrap();
        let elements = result.as_list_elements().unwrap();
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].as_integer().unwrap(), 1);
    }

    #[test]
    fn parse_text_content_option_some_string() {
        let result = parse_text_content("\"hello\"", &Type::option(Type::string())).unwrap();
        let inner = result.as_option().unwrap().unwrap();
        assert_eq!(inner.as_string().unwrap(), "hello");
    }
}
