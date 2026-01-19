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
}

impl ExternalFunctionDefinition {
    pub fn new(name: String, parameters: Vec<(String, Type)>, return_type: Type) -> Self {
        Self {
            name,
            parameters,
            return_type,
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
        context: &mut crate::runtime::Context,
    ) -> Result<crate::runtime::ExprResult, String>;
    fn return_type(&self) -> Type;
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Expression>;
}

#[async_trait(?Send)]
pub trait ExecutableFunction: Function + Expression + std::fmt::Debug {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction>;
}

#[async_trait(?Send)]
pub trait LanguageEngine {
    async fn untyped(&self, context: &crate::runtime::Context) -> String;
}

pub struct PrintEngine {}

#[async_trait(?Send)]
impl LanguageEngine for PrintEngine {
    async fn untyped(&self, _context: &crate::runtime::Context) -> String {
        println!("PrintEngine {{}}");
        "PrintEngine {}".to_string()
    }
}
