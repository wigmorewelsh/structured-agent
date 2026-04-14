use async_trait::async_trait;
use std::any::Any;
use std::sync::{Arc, Mutex};

pub use structured_agent_runtime::{ExternalFunctionDefinition, NativeFunction, Parameter, Type};

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
    inner: Arc<Mutex<codespan_reporting::files::SimpleFiles<String, String>>>,
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
            inner: Arc::new(Mutex::new(codespan_reporting::files::SimpleFiles::new())),
        }
    }

    pub fn add(&self, name: String, source: String) -> FileId {
        self.inner.lock().unwrap().add(name, source)
    }

    pub fn files(&self) -> Arc<Mutex<codespan_reporting::files::SimpleFiles<String, String>>> {
        self.inner.clone()
    }
}

#[async_trait]
pub trait Function: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn parameters(&self) -> &[Parameter];
    fn function_return_type(&self) -> &Type;
    async fn execute(
        &self,
        context: crate::runtime::Context,
        args: Vec<crate::runtime::ExpressionResult>,
    ) -> Result<(crate::runtime::Context, crate::runtime::ExpressionResult), String>;
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Function>;
    fn documentation(&self) -> Option<&str> {
        None
    }
}

#[async_trait]
pub trait ExecutableFunction: Function + std::fmt::Debug + Send + Sync {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction>;
}

#[async_trait]
pub trait LanguageEngine: Send + Sync {
    async fn untyped(&self, context: &crate::runtime::Context) -> String;
    async fn typed(
        &self,
        context: &crate::runtime::Context,
        return_type: &Type,
    ) -> Result<crate::runtime::ExpressionValue, String>;
    async fn select(
        &self,
        context: &crate::runtime::Context,
        options: &[crate::runtime::ExpressionValue],
    ) -> Result<usize, String>;
    async fn fill_parameter(
        &self,
        context: &crate::runtime::Context,
        param_name: &str,
        param_type: &Type,
    ) -> Result<crate::runtime::ExpressionValue, String>;
}

pub struct PrintEngine {}

impl PrintEngine {
    fn format_event(event: &crate::runtime::Event) -> String {
        let content = event.content.format_for_llm();

        if let Some(name) = &event.name {
            let params_xml = if let Some(params) = &event.params {
                let params_str = params
                    .iter()
                    .map(|p| {
                        let value = p.value.format_for_llm();
                        format!("    <param name=\"{}\">{}</param>", p.name, value)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{}\n", params_str)
            } else {
                String::new()
            };

            format!(
                "<{}>\n{}    <result>\n    {}\n    </result>\n</{}>",
                name, params_xml, content, name
            )
        } else {
            content
        }
    }
}

#[async_trait]
impl LanguageEngine for PrintEngine {
    async fn untyped(&self, context: &crate::runtime::Context) -> String {
        if let Some(last_event) = context.last_event() {
            Self::format_event(&last_event)
        } else {
            "PrintEngine {}".to_string()
        }
    }

    async fn typed(
        &self,
        context: &crate::runtime::Context,
        return_type: &Type,
    ) -> Result<crate::runtime::ExpressionValue, String> {
        match return_type {
            Type::String => {
                let value = self.untyped(context).await;
                Ok(crate::runtime::ExpressionValue::string(value))
            }
            Type::Boolean => Ok(crate::runtime::ExpressionValue::boolean(true)),
            Type::Int => Ok(crate::runtime::ExpressionValue::integer(0)),
            Type::Unit => Ok(crate::runtime::ExpressionValue::unit()),
            Type::Parameterized(n, args) => {
                if n.name == "Option" {
                    Ok(crate::runtime::ExpressionValue::option_none_with_type(
                        context.runtime().type_to_arrow_datatype(&args[0]),
                    ))
                } else {
                    let value = self.untyped(context).await;
                    Ok(crate::runtime::ExpressionValue::string(value))
                }
            }
            Type::Struct(_) => Ok(crate::runtime::ExpressionValue::unit()),
            Type::Generic(_) => Ok(crate::runtime::ExpressionValue::unit()),
        }
    }

    async fn select(
        &self,
        _context: &crate::runtime::Context,
        _options: &[crate::runtime::ExpressionValue],
    ) -> Result<usize, String> {
        Ok(0)
    }

    async fn fill_parameter(
        &self,
        context: &crate::runtime::Context,
        _param_name: &str,
        param_type: &Type,
    ) -> Result<crate::runtime::ExpressionValue, String> {
        match param_type {
            Type::String => {
                let value = self.untyped(context).await;
                Ok(crate::runtime::ExpressionValue::string(value))
            }
            Type::Boolean => Ok(crate::runtime::ExpressionValue::boolean(true)),
            Type::Int => Ok(crate::runtime::ExpressionValue::integer(0)),
            Type::Parameterized(n, args) => {
                if n.name == "Option" {
                    Ok(crate::runtime::ExpressionValue::option_none_with_type(
                        context.runtime().type_to_arrow_datatype(&args[0]),
                    ))
                } else {
                    let value = self.untyped(context).await;
                    Ok(crate::runtime::ExpressionValue::string(value))
                }
            }
            Type::Unit | Type::Struct(_) | Type::Generic(_) => {
                Ok(crate::runtime::ExpressionValue::unit())
            }
        }
    }
}

#[async_trait]
pub trait FunctionProvider: Send + Sync {
    async fn list_functions(
        &self,
    ) -> Result<Vec<ExternalFunctionDefinition>, crate::runtime::RuntimeError>;
    async fn create_expression(
        &self,
        definition: &ExternalFunctionDefinition,
    ) -> Result<std::sync::Arc<dyn ExecutableFunction>, crate::runtime::RuntimeError>;
}
