use crate::expressions::ExternalFunctionExpr;
use crate::runtime::RuntimeService;
use crate::runtime::{ExpressionValue, RuntimeError};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, FunctionProvider, Parameter, Type,
};
use arrow::array::Array;
use arrow::datatypes::{DataType, Field};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rmcp::model::{CallToolRequestParams, RawAudioContent, RawImageContent, RawResource, Tool};
use rmcp::{RoleClient, ServiceError, ServiceExt};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use structured_agent_runtime::symbols::DefinitionPath;
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

fn expression_to_json(value: &ExpressionValue) -> Value {
    if let Ok(s) = value.as_string() {
        return Value::String(s);
    }
    if let Ok(n) = value.as_integer() {
        return Value::Number(n.into());
    }
    if let Ok(b) = value.as_boolean() {
        return Value::Bool(b);
    }
    if value.type_name() == "Unit" {
        return Value::Null;
    }
    if let Ok(fields) = value.as_struct_fields() {
        let mut map = serde_json::Map::new();
        for (name, val) in fields {
            map.insert(name, expression_to_json(&val));
        }
        return Value::Object(map);
    }
    if value.is_option() {
        return match value.as_option() {
            Ok(Some(inner)) => expression_to_json(&inner),
            _ => Value::Null,
        };
    }
    if let Ok(elements) = value.as_list_elements() {
        return Value::Array(elements.iter().map(expression_to_json).collect());
    }
    Value::Null
}

fn serialise_args(args: &[(String, ExpressionValue)]) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    for (name, value) in args {
        map.insert(name.clone(), expression_to_json(value));
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
    } else if let Type::Parameterized(_, args) = ty
        && let Some(inner) = args.first()
    {
        DataType::List(Arc::new(Field::new("item", ty_to_datatype(inner), true)))
    } else {
        DataType::Null
    }
}

fn json_to_expression(
    json: &serde_json::Value,
    ty: &Type,
    runtime: &dyn RuntimeService,
) -> Result<ExpressionValue, McpError> {
    if ty.is_string() {
        return Ok(ExpressionValue::string(
            json.as_str().unwrap_or(&json.to_string()).to_string(),
        ));
    }
    if ty.is_int() {
        let n = json
            .as_i64()
            .or_else(|| json.as_str().and_then(|s| s.parse::<i64>().ok()))
            .ok_or_else(|| McpError::ToolError(format!("Cannot parse {:?} as integer", json)))?;
        return Ok(ExpressionValue::integer(n));
    }
    if ty.is_boolean() {
        let b = json
            .as_bool()
            .or_else(|| json.as_str().and_then(|s| s.parse::<bool>().ok()))
            .ok_or_else(|| McpError::ToolError(format!("Cannot parse {:?} as boolean", json)))?;
        return Ok(ExpressionValue::boolean(b));
    }
    if ty.is_unit() {
        return Ok(ExpressionValue::unit());
    }
    if ty.is_list()
        && let Type::Parameterized(_, args) = ty
        && let Some(inner) = args.first()
    {
        let arr = json
            .as_array()
            .ok_or_else(|| McpError::ToolError(format!("Expected JSON array, got {:?}", json)))?;
        let elements: Result<Vec<ExpressionValue>, McpError> = arr
            .iter()
            .map(|v| json_to_expression(v, inner, runtime))
            .collect();
        return ExpressionValue::from_elements(elements?).map_err(McpError::ToolError);
    }
    if ty.is_option()
        && let Type::Parameterized(_, args) = ty
        && let Some(inner) = args.first()
    {
        if json.is_null() {
            return Ok(ExpressionValue::option_none_with_type(ty_to_datatype(
                inner,
            )));
        }
        return json_to_expression(json, inner, runtime).map(ExpressionValue::option_some);
    }
    if let Type::Named(path) = ty
        && let Some(fields) = runtime.get_struct(path)
    {
        let obj = json.as_object().ok_or_else(|| {
            McpError::ToolError(format!(
                "Expected JSON object for type '{}', got {:?}",
                ty.name(),
                json
            ))
        })?;
        let struct_fields: Result<Vec<(&str, ExpressionValue)>, McpError> = fields
            .iter()
            .map(|(name, field_ty)| {
                let field_json = obj.get(name).unwrap_or(&serde_json::Value::Null);
                json_to_expression(field_json, field_ty, runtime).map(|v| (name.as_str(), v))
            })
            .collect();
        return Ok(ExpressionValue::struct_value(struct_fields?));
    }
    Err(McpError::ToolError(format!(
        "Type '{}' must be explicitly declared in SA; user-defined types are not supported for external function calls",
        ty.name()
    )))
}

fn handle_single_content_block(
    raw: &rmcp::model::RawContent,
    return_type: &Type,
    runtime: &dyn RuntimeService,
) -> Result<ExpressionValue, McpError> {
    match raw {
        rmcp::model::RawContent::Text(text_content) => {
            parse_text_content(&text_content.text, return_type, runtime)
        }
        other => Ok(raw_content_to_expression_value(other)),
    }
}

fn parse_text_content(
    text: &str,
    return_type: &Type,
    runtime: &dyn RuntimeService,
) -> Result<ExpressionValue, McpError> {
    if return_type.is_string() {
        return Ok(ExpressionValue::string(text.to_string()));
    }
    let json: serde_json::Value = serde_json::from_str(text)
        .map_err(|_| McpError::ToolError(format!("Cannot parse {:?} as JSON", text)))?;
    json_to_expression(&json, return_type, runtime)
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
        runtime: &dyn RuntimeService,
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
            let msg = response
                .content
                .first()
                .and_then(|block| match &block.raw {
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
                elements.push(raw_content_to_expression_value(&block.raw));
            }
            return ExpressionValue::from_elements(elements).map_err(McpError::ToolError);
        }

        handle_single_content_block(&response.content[0].raw, return_type, runtime)
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
                                    json_schema_to_type(v)
                                        .unwrap_or_else(|| Type::Generic("_".to_string())),
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

pub fn raw_content_to_expression_value(raw: &rmcp::model::RawContent) -> ExpressionValue {
    match raw {
        rmcp::model::RawContent::Image(img) => {
            let bytes = STANDARD.decode(&img.data).unwrap_or_default();
            ExpressionValue::image(img.mime_type.clone(), bytes)
        }
        rmcp::model::RawContent::Audio(aud) => {
            let bytes = STANDARD.decode(&aud.data).unwrap_or_default();
            ExpressionValue::audio(aud.mime_type.clone(), bytes)
        }
        rmcp::model::RawContent::ResourceLink(r) => {
            ExpressionValue::link(r.uri.clone(), Some(r.name.clone()))
        }
        rmcp::model::RawContent::Text(t) => ExpressionValue::string(t.text.clone()),
        other => ExpressionValue::string(format!("{:?}", other)),
    }
}

pub fn expression_value_to_raw_content(value: &ExpressionValue) -> rmcp::model::RawContent {
    if let Ok(img) = value.as_image() {
        let b64 = STANDARD.encode(&img.data);
        return rmcp::model::RawContent::Image(RawImageContent {
            data: b64,
            mime_type: img.mime_type.clone(),
            meta: None,
        });
    }
    if let Ok(aud) = value.as_audio() {
        let b64 = STANDARD.encode(&aud.data);
        return rmcp::model::RawContent::Audio(RawAudioContent {
            data: b64,
            mime_type: aud.mime_type.clone(),
        });
    }
    if let Ok(link) = value.as_link() {
        let display_name = link.name.as_deref().unwrap_or(&link.uri).to_string();
        return rmcp::model::RawContent::ResourceLink(RawResource::new(
            link.uri.clone(),
            display_name,
        ));
    }
    rmcp::model::RawContent::text(value.value_string())
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
    use crate::runtime::{ExpressionValue, RuntimeService};
    use serde_json::json;

    struct NoStructRuntime;

    impl RuntimeService for NoStructRuntime {
        fn get_native_function(
            &self,
            _: &str,
        ) -> Option<std::sync::Arc<dyn crate::types::ExecutableFunction>> {
            None
        }
        fn get_bytecode_ref(&self, _: &DefinitionPath) -> Option<structured_agent_il::BytecodeRef> {
            None
        }
        fn engine(&self) -> &dyn crate::types::LanguageEngine {
            unimplemented!()
        }
        fn type_to_arrow_datatype(&self, _: &Type) -> arrow::datatypes::DataType {
            arrow::datatypes::DataType::Null
        }
        fn get_struct(&self, _: &DefinitionPath) -> Option<Vec<(String, Type)>> {
            None
        }
        fn get_struct_with_args(
            &self,
            _: &DefinitionPath,
            _: &[Type],
        ) -> Option<Vec<(String, Type)>> {
            None
        }
    }

    fn no_struct() -> &'static NoStructRuntime {
        &NoStructRuntime
    }

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
                no_struct(),
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
        let result = parse_text_content("hello", &Type::string(), no_struct());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_string().unwrap(), "hello");
    }

    #[test]
    fn parse_text_content_integer() {
        let result = parse_text_content("42", &Type::int(), no_struct());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_integer().unwrap(), 42);
    }

    #[test]
    fn parse_text_content_integer_invalid() {
        let result = parse_text_content("abc", &Type::int(), no_struct());
        assert!(result.is_err());
    }

    #[test]
    fn parse_text_content_bool_true() {
        let result = parse_text_content("true", &Type::boolean(), no_struct());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_boolean().unwrap(), true);
    }

    #[test]
    fn parse_text_content_bool_false() {
        let result = parse_text_content("false", &Type::boolean(), no_struct());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_boolean().unwrap(), false);
    }

    #[test]
    fn parse_text_content_bool_invalid() {
        let result = parse_text_content("yes", &Type::boolean(), no_struct());
        assert!(result.is_err());
    }

    #[test]
    fn parse_text_content_list_of_strings() {
        let result = parse_text_content(
            "[\"a\",\"b\",\"c\"]",
            &Type::list(Type::string()),
            no_struct(),
        );
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
        let value = result.unwrap();
        let list = value.as_list().unwrap();
        assert_eq!(list.value(0).len(), 3);
    }

    #[test]
    fn parse_text_content_empty_list() {
        let result = parse_text_content("[]", &Type::list(Type::string()), no_struct());
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
        let value = result.unwrap();
        let list = value.as_list().unwrap();
        assert_eq!(list.value(0).len(), 0);
    }

    #[test]
    fn parse_text_content_list_invalid_json() {
        let result = parse_text_content("not json", &Type::list(Type::string()), no_struct());
        assert!(result.is_err());
    }

    #[test]
    fn parse_text_content_list_not_array() {
        let result = parse_text_content(
            "{\"key\":\"val\"}",
            &Type::list(Type::string()),
            no_struct(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn call_tool_serialises_unit_arg_as_null() {
        let result = serialise_args(&[("x".to_string(), ExpressionValue::unit())]);
        assert_eq!(serde_json::Value::Object(result), json!({"x": null}));
    }

    #[test]
    fn json_to_expression_string() {
        let result =
            json_to_expression(&serde_json::json!("hi"), &Type::string(), no_struct()).unwrap();
        assert_eq!(result.as_string().unwrap(), "hi");
    }

    #[test]
    fn json_to_expression_int() {
        let result = json_to_expression(&serde_json::json!(42), &Type::int(), no_struct()).unwrap();
        assert_eq!(result.as_integer().unwrap(), 42);
    }

    #[test]
    fn json_to_expression_bool() {
        let result =
            json_to_expression(&serde_json::json!(true), &Type::boolean(), no_struct()).unwrap();
        assert!(result.as_boolean().unwrap());
    }

    #[test]
    fn json_to_expression_list_of_int() {
        let result = json_to_expression(
            &serde_json::json!([1, 2, 3]),
            &Type::list(Type::int()),
            no_struct(),
        )
        .unwrap();
        let elements = result.as_list_elements().unwrap();
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].as_integer().unwrap(), 1);
        assert_eq!(elements[1].as_integer().unwrap(), 2);
        assert_eq!(elements[2].as_integer().unwrap(), 3);
    }

    #[test]
    fn json_to_expression_list_of_bool() {
        let result = json_to_expression(
            &serde_json::json!([true, false]),
            &Type::list(Type::boolean()),
            no_struct(),
        )
        .unwrap();
        let elements = result.as_list_elements().unwrap();
        assert_eq!(elements.len(), 2);
        assert!(elements[0].as_boolean().unwrap());
        assert!(!elements[1].as_boolean().unwrap());
    }

    #[test]
    fn json_to_expression_option_some() {
        let result = json_to_expression(
            &serde_json::json!("x"),
            &Type::option(Type::string()),
            no_struct(),
        )
        .unwrap();
        let inner = result.as_option().unwrap().unwrap();
        assert_eq!(inner.as_string().unwrap(), "x");
    }

    #[test]
    fn json_to_expression_option_none() {
        let result = json_to_expression(
            &serde_json::Value::Null,
            &Type::option(Type::string()),
            no_struct(),
        )
        .unwrap();
        assert!(result.as_option().unwrap().is_none());
    }

    #[test]
    fn json_to_expression_unknown_type_is_error() {
        use nonempty::NonEmpty;
        let ty = Type::Named(DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new("main".to_string())),
            "Person",
        ));
        let result = json_to_expression(&serde_json::json!({"name": "alice"}), &ty, no_struct());
        assert!(result.is_err());
    }

    #[test]
    fn json_to_expression_struct_with_resolver() {
        use nonempty::NonEmpty;

        struct PersonRuntime;
        impl RuntimeService for PersonRuntime {
            fn get_native_function(
                &self,
                _: &str,
            ) -> Option<std::sync::Arc<dyn crate::types::ExecutableFunction>> {
                None
            }
            fn get_bytecode_ref(
                &self,
                _: &DefinitionPath,
            ) -> Option<structured_agent_il::BytecodeRef> {
                None
            }
            fn engine(&self) -> &dyn crate::types::LanguageEngine {
                unimplemented!()
            }
            fn type_to_arrow_datatype(&self, _: &Type) -> arrow::datatypes::DataType {
                arrow::datatypes::DataType::Null
            }
            fn get_struct(&self, p: &DefinitionPath) -> Option<Vec<(String, Type)>> {
                if p.last_name() == "Person" {
                    Some(vec![
                        ("name".to_string(), Type::string()),
                        ("age".to_string(), Type::int()),
                    ])
                } else {
                    None
                }
            }
            fn get_struct_with_args(
                &self,
                _: &DefinitionPath,
                _: &[Type],
            ) -> Option<Vec<(String, Type)>> {
                None
            }
        }

        let path = DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new("main".to_string())),
            "Person",
        );
        let ty = Type::Named(path.clone());
        let result = json_to_expression(
            &serde_json::json!({"name": "alice", "age": 30}),
            &ty,
            &PersonRuntime,
        )
        .unwrap();
        assert_eq!(
            result
                .get_struct_field("name")
                .unwrap()
                .as_string()
                .unwrap(),
            "alice"
        );
        assert_eq!(
            result
                .get_struct_field("age")
                .unwrap()
                .as_integer()
                .unwrap(),
            30
        );
    }

    #[test]
    fn json_to_expression_unknown_type_with_no_resolver_is_error() {
        use nonempty::NonEmpty;
        let ty = Type::Named(DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new("main".to_string())),
            "UnknownType",
        ));
        let result = json_to_expression(&serde_json::json!({"x": 1}), &ty, no_struct());
        assert!(result.is_err());
    }

    #[test]
    fn parse_text_content_list_of_int() {
        let result = parse_text_content("[1,2,3]", &Type::list(Type::int()), no_struct()).unwrap();
        let elements = result.as_list_elements().unwrap();
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].as_integer().unwrap(), 1);
    }

    #[test]
    fn serialise_args_list_of_int() {
        let list = ExpressionValue::from_elements(vec![
            ExpressionValue::integer(1),
            ExpressionValue::integer(2),
            ExpressionValue::integer(3),
        ])
        .unwrap();
        let map = serialise_args(&[("items".to_string(), list)]);
        assert_eq!(map["items"], serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn serialise_args_list_of_bool() {
        let list = ExpressionValue::from_elements(vec![
            ExpressionValue::boolean(true),
            ExpressionValue::boolean(false),
        ])
        .unwrap();
        let map = serialise_args(&[("flags".to_string(), list)]);
        assert_eq!(map["flags"], serde_json::json!([true, false]));
    }

    #[test]
    fn serialise_args_option_some_string() {
        let opt = ExpressionValue::option_some(ExpressionValue::string("x"));
        let map = serialise_args(&[("val".to_string(), opt)]);
        assert_eq!(map["val"], serde_json::json!("x"));
    }

    #[test]
    fn serialise_args_option_none() {
        let opt = ExpressionValue::option_none_utf8();
        let map = serialise_args(&[("val".to_string(), opt)]);
        assert_eq!(map["val"], serde_json::Value::Null);
    }

    #[test]
    fn parse_text_content_option_some_string() {
        let result =
            parse_text_content("\"hello\"", &Type::option(Type::string()), no_struct()).unwrap();
        let inner = result.as_option().unwrap().unwrap();
        assert_eq!(inner.as_string().unwrap(), "hello");
    }

    #[test]
    fn call_tool_single_image_block_returns_image_value() {
        let raw_bytes = b"pixels";
        let b64 = STANDARD.encode(raw_bytes);
        let raw = rmcp::model::RawContent::Image(RawImageContent {
            data: b64,
            mime_type: "image/png".to_string(),
            meta: None,
        });
        let result = handle_single_content_block(&raw, &Type::string(), no_struct());
        let img = result.unwrap().as_image().unwrap();
        assert_eq!(img.mime_type, "image/png");
        assert_eq!(img.data, raw_bytes);
    }

    #[test]
    fn call_tool_single_audio_block_returns_audio_value() {
        let raw_bytes = b"sounddata";
        let b64 = STANDARD.encode(raw_bytes);
        let raw = rmcp::model::RawContent::Audio(RawAudioContent {
            data: b64,
            mime_type: "audio/mp3".to_string(),
        });
        let result = handle_single_content_block(&raw, &Type::string(), no_struct());
        let aud = result.unwrap().as_audio().unwrap();
        assert_eq!(aud.mime_type, "audio/mp3");
        assert_eq!(aud.data, raw_bytes);
    }

    #[test]
    fn call_tool_single_resource_link_returns_link_value() {
        let raw = rmcp::model::RawContent::ResourceLink(RawResource::new(
            "https://example.com/doc",
            "doc",
        ));
        let result = handle_single_content_block(&raw, &Type::string(), no_struct());
        let link = result.unwrap().as_link().unwrap();
        assert_eq!(link.uri, "https://example.com/doc");
        assert_eq!(link.name, Some("doc".to_string()));
    }

    #[test]
    fn mcp_image_content_converts_to_image_value() {
        let raw_bytes = b"hello image";
        let b64 = STANDARD.encode(raw_bytes);
        let raw = rmcp::model::RawContent::Image(RawImageContent {
            data: b64,
            mime_type: "image/png".to_string(),
            meta: None,
        });
        let value = raw_content_to_expression_value(&raw);
        let img = value.as_image().unwrap();
        assert_eq!(img.mime_type, "image/png");
        assert_eq!(img.data, raw_bytes);
    }

    #[test]
    fn mcp_audio_content_converts_to_audio_value() {
        let raw_bytes = b"hello audio";
        let b64 = STANDARD.encode(raw_bytes);
        let raw = rmcp::model::RawContent::Audio(RawAudioContent {
            data: b64,
            mime_type: "audio/mp3".to_string(),
        });
        let value = raw_content_to_expression_value(&raw);
        let aud = value.as_audio().unwrap();
        assert_eq!(aud.mime_type, "audio/mp3");
        assert_eq!(aud.data, raw_bytes);
    }

    #[test]
    fn mcp_resource_link_converts_to_link_value() {
        let raw = rmcp::model::RawContent::ResourceLink(RawResource::new(
            "https://example.com/logo.png",
            "logo.png",
        ));
        let value = raw_content_to_expression_value(&raw);
        let link = value.as_link().unwrap();
        assert_eq!(link.uri, "https://example.com/logo.png");
        assert_eq!(link.name, Some("logo.png".to_string()));
    }

    #[test]
    fn image_value_converts_to_mcp_image_content() {
        let value = ExpressionValue::image("image/png", b"abc".to_vec());
        let raw = expression_value_to_raw_content(&value);
        let rmcp::model::RawContent::Image(img) = raw else {
            panic!("Expected Image content");
        };
        assert_eq!(img.mime_type, "image/png");
        let decoded = STANDARD.decode(&img.data).unwrap();
        assert_eq!(decoded, b"abc");
    }

    #[test]
    fn audio_value_converts_to_mcp_audio_content() {
        let value = ExpressionValue::audio("audio/wav", b"xyz".to_vec());
        let raw = expression_value_to_raw_content(&value);
        let rmcp::model::RawContent::Audio(aud) = raw else {
            panic!("Expected Audio content");
        };
        assert_eq!(aud.mime_type, "audio/wav");
        let decoded = STANDARD.decode(&aud.data).unwrap();
        assert_eq!(decoded, b"xyz");
    }

    #[test]
    fn link_value_converts_to_mcp_resource_link() {
        let value =
            ExpressionValue::link("https://example.com/logo.png", Some("logo.png".to_string()));
        let raw = expression_value_to_raw_content(&value);
        let rmcp::model::RawContent::ResourceLink(r) = raw else {
            panic!("Expected ResourceLink content");
        };
        assert_eq!(r.uri, "https://example.com/logo.png");
        assert_eq!(r.name, "logo.png");
    }
}
