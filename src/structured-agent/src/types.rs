use async_trait::async_trait;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

pub type FileId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn dummy() -> Self {
        Self { start: 0, end: 0 }
    }

    pub fn to_byte_range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

#[derive(Debug, Clone)]
pub struct SourceFiles {
    inner: Rc<RefCell<codespan_reporting::files::SimpleFiles<String, String>>>,
}

pub trait Spanned {
    fn span(&self) -> Span;
}

impl Default for SourceFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceFiles {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(codespan_reporting::files::SimpleFiles::new())),
        }
    }

    pub fn add(&self, name: String, source: String) -> FileId {
        self.inner.borrow_mut().add(name, source)
    }

    pub fn files(&self) -> Rc<RefCell<codespan_reporting::files::SimpleFiles<String, String>>> {
        self.inner.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    String,
    Boolean,
    Unit,
    Custom(String),
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

    pub fn custom(name: String) -> Self {
        Self::Custom(name)
    }

    pub fn name(&self) -> String {
        match self {
            Type::String => "String".to_string(),
            Type::Boolean => "Boolean".to_string(),
            Type::Unit => "()".to_string(),
            Type::Custom(name) => name.clone(),
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

#[async_trait(?Send)]
pub trait Function: std::fmt::Debug {
    fn name(&self) -> &str;
    fn parameters(&self) -> &[Parameter];
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
        match return_type {
            Type::String => {
                let value = self.untyped(context).await;
                Ok(crate::runtime::ExprResult::String(value))
            }
            Type::Boolean => Ok(crate::runtime::ExprResult::Boolean(true)),
            Type::Unit => Ok(crate::runtime::ExprResult::Unit),
            Type::Custom(_) => {
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
        match param_type {
            Type::String => {
                let value = self.untyped(context).await;
                Ok(crate::runtime::ExprResult::String(value))
            }
            Type::Boolean => Ok(crate::runtime::ExprResult::Boolean(true)),
            Type::Unit | Type::Custom(_) => Ok(crate::runtime::ExprResult::String(format!(
                "PrintEngine: {} ({})",
                param_name,
                param_type.name()
            ))),
        }
    }
}

#[async_trait(?Send)]
pub trait NativeFunction: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn parameters(&self) -> &[Parameter];
    fn return_type(&self) -> &Type;
    async fn execute(
        &self,
        args: Vec<crate::runtime::ExprResult>,
    ) -> Result<crate::runtime::ExprResult, String>;
    fn documentation(&self) -> Option<&str> {
        None
    }
}
