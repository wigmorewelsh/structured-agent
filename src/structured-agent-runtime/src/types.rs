use async_trait::async_trait;

use crate::actor::AgentHandle;
use crate::expression::ExpressionValue;
use crate::symbols::TypeName;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    String,
    Boolean,
    Int,
    Unit,
    Struct(TypeName),
    List(Box<Type>),
    Option(Box<Type>),
    Generic(std::string::String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalFunctionDefinition {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub documentation: Option<String>,
}

impl Type {
    pub fn string() -> Self {
        Self::String
    }

    pub fn unit() -> Self {
        Self::Unit
    }

    pub fn boolean() -> Self {
        Self::Boolean
    }

    pub fn int() -> Self {
        Self::Int
    }

    pub fn list(inner: Type) -> Self {
        Self::List(Box::new(inner))
    }

    pub fn option(inner: Type) -> Self {
        Self::Option(Box::new(inner))
    }

    pub fn generic(name: impl Into<std::string::String>) -> Self {
        Self::Generic(name.into())
    }

    pub fn name(&self) -> String {
        match self {
            Type::String => "String".to_string(),
            Type::Boolean => "Boolean".to_string(),
            Type::Int => "Int".to_string(),
            Type::Unit => "()".to_string(),
            Type::Struct(tn) => tn.name.clone(),
            Type::List(inner) => format!("List<{}>", inner.name()),
            Type::Option(inner) => format!("Option<{}>", inner.name()),
            Type::Generic(name) => name.clone(),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::String => write!(f, "String"),
            Type::Boolean => write!(f, "Boolean"),
            Type::Int => write!(f, "Int"),
            Type::Unit => write!(f, "Unit"),
            Type::Struct(tn) => write!(f, "{}", tn.name),
            Type::List(inner) => write!(f, "List<{}>", inner),
            Type::Option(inner) => write!(f, "Option<{}>", inner),
            Type::Generic(name) => write!(f, "{}", name),
        }
    }
}

impl Parameter {
    pub fn new(name: String, param_type: Type) -> Self {
        Self { name, param_type }
    }
}

impl ExternalFunctionDefinition {
    pub fn new(name: String, parameters: Vec<Parameter>, return_type: Type) -> Self {
        Self {
            name,
            parameters,
            return_type,
            documentation: None,
        }
    }

    pub fn new_with_docs(
        name: String,
        parameters: Vec<Parameter>,
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

#[async_trait]
pub trait NativeFunction: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn parameters(&self) -> &[Parameter];
    fn return_type(&self) -> &Type;
    async fn execute(
        &self,
        args: Vec<ExpressionValue>,
        agent: &AgentHandle,
    ) -> Result<ExpressionValue, String>;
    fn documentation(&self) -> Option<&str> {
        None
    }

    fn type_params(&self) -> &[String] {
        &[]
    }
}

pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn functions(&self) -> Vec<std::sync::Arc<dyn NativeFunction>>;
}
