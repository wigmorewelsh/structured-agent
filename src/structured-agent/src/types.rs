use async_trait::async_trait;
use std::any::Any;

#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalFunctionDefinition {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub return_type: Type,
    pub documentation: Option<String>,
}

impl Type {
    pub fn string() -> Self {
        Self {
            name: "String".to_string(),
        }
    }

    pub fn unit() -> Self {
        Self {
            name: "()".to_string(),
        }
    }

    pub fn boolean() -> Self {
        Self {
            name: "Boolean".to_string(),
        }
    }
}

impl ExternalFunctionDefinition {
    pub fn new(name: String, parameters: Vec<(String, Type)>, return_type: Type) -> Self {
        Self {
            name,
            parameters,
            return_type,
            documentation: None,
        }
    }

    pub fn new_with_docs(
        name: String,
        parameters: Vec<(String, Type)>,
        return_type: Type,
        documentation: Option<String>,
    ) -> Self {
        Self {
            name,
            parameters,
            return_type,
            documentation,
        }
    }
}

#[async_trait(?Send)]
pub trait Function: std::fmt::Debug {
    fn name(&self) -> &str;
    fn parameters(&self) -> &[(String, Type)];
    fn function_return_type(&self) -> &Type;
}

#[async_trait(?Send)]
pub trait Expression: std::fmt::Debug {
    async fn evaluate(
        &self,
        context: std::sync::Arc<crate::runtime::Context>,
    ) -> Result<crate::runtime::ExprResult, String>;
    fn return_type(&self) -> Type;
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Expression>;
    fn documentation(&self) -> Option<&str> {
        None
    }
}

#[async_trait(?Send)]
pub trait ExecutableFunction: Function + Expression + std::fmt::Debug {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction>;
}

#[async_trait(?Send)]
pub trait LanguageEngine {
    async fn untyped(&self, context: &crate::runtime::Context) -> String;
    async fn typed(
        &self,
        context: &crate::runtime::Context,
        return_type: &Type,
    ) -> Result<crate::runtime::ExprResult, String>;
    async fn select(
        &self,
        context: &crate::runtime::Context,
        options: &[String],
    ) -> Result<usize, String>;
    async fn fill_parameter(
        &self,
        context: &crate::runtime::Context,
        param_name: &str,
        param_type: &Type,
    ) -> Result<crate::runtime::ExprResult, String>;
}

pub struct PrintEngine {}

#[async_trait(?Send)]
impl LanguageEngine for PrintEngine {
    async fn untyped(&self, context: &crate::runtime::Context) -> String {
        if let Some(last_event) = context.last_event() {
            last_event.message.clone()
        } else {
            "PrintEngine {}".to_string()
        }
    }

    async fn typed(
        &self,
        context: &crate::runtime::Context,
        return_type: &Type,
    ) -> Result<crate::runtime::ExprResult, String> {
        match return_type.name.as_str() {
            "String" => {
                let value = self.untyped(context).await;
                Ok(crate::runtime::ExprResult::String(value))
            }
            "Boolean" => Ok(crate::runtime::ExprResult::Boolean(true)),
            "()" => Ok(crate::runtime::ExprResult::Unit),
            _ => {
                let value = self.untyped(context).await;
                Ok(crate::runtime::ExprResult::String(value))
            }
        }
    }

    async fn select(
        &self,
        _context: &crate::runtime::Context,
        _options: &[String],
    ) -> Result<usize, String> {
        Ok(0)
    }

    async fn fill_parameter(
        &self,
        context: &crate::runtime::Context,
        param_name: &str,
        param_type: &Type,
    ) -> Result<crate::runtime::ExprResult, String> {
        match param_type.name.as_str() {
            "String" => {
                let value = self.untyped(context).await;
                Ok(crate::runtime::ExprResult::String(value))
            }
            "Boolean" => Ok(crate::runtime::ExprResult::Boolean(true)),
            _ => Ok(crate::runtime::ExprResult::String(format!(
                "PrintEngine: {} ({})",
                param_name, param_type.name
            ))),
        }
    }
}

#[async_trait(?Send)]
pub trait NativeFunction: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn parameters(&self) -> &[(String, Type)];
    fn return_type(&self) -> &Type;
    async fn execute(
        &self,
        args: Vec<crate::runtime::ExprResult>,
    ) -> Result<crate::runtime::ExprResult, String>;
    fn documentation(&self) -> Option<&str> {
        None
    }
}
