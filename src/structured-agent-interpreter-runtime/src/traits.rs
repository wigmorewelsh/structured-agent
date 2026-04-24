use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use structured_agent_runtime::{
    ExpressionResult, ExpressionValue, ExternalFunctionDefinition, Parameter, RuntimeError, Type,
};

use crate::context::{Context, Event};

#[async_trait]
pub trait Function: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn parameters(&self) -> &[Parameter];
    fn function_return_type(&self) -> &Type;
    async fn execute(
        &self,
        context: Context,
        args: Vec<ExpressionResult>,
    ) -> Result<(Context, ExpressionResult), String>;
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
    async fn untyped(&self, context: &Context) -> String;
    async fn typed(&self, context: &Context, return_type: &Type)
    -> Result<ExpressionValue, String>;
    async fn select(&self, context: &Context, options: &[ExpressionValue])
    -> Result<usize, String>;
    async fn fill_parameter(
        &self,
        context: &Context,
        param_name: &str,
        param_type: &Type,
    ) -> Result<ExpressionValue, String>;
}

#[async_trait]
pub trait FunctionProvider: Send + Sync {
    async fn list_functions(&self) -> Result<Vec<ExternalFunctionDefinition>, RuntimeError>;
    async fn create_expression(
        &self,
        definition: &ExternalFunctionDefinition,
    ) -> Result<Arc<dyn ExecutableFunction>, RuntimeError>;
}

pub fn format_event(event: &Event) -> String {
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

pub struct PrintEngine {}

#[async_trait]
impl LanguageEngine for PrintEngine {
    async fn untyped(&self, context: &Context) -> String {
        if let Some(last_event) = context.last_event() {
            format_event(&last_event)
        } else {
            "PrintEngine {}".to_string()
        }
    }

    async fn typed(
        &self,
        context: &Context,
        return_type: &Type,
    ) -> Result<ExpressionValue, String> {
        match return_type {
            _ if return_type.is_string() => {
                let value = self.untyped(context).await;
                Ok(ExpressionValue::string(value))
            }
            _ if return_type.is_boolean() => Ok(ExpressionValue::boolean(true)),
            _ if return_type.is_int() => Ok(ExpressionValue::integer(0)),
            _ if return_type.is_unit() => Ok(ExpressionValue::unit()),
            Type::Parameterized(n, args) => {
                if n.last_name() == "Option" {
                    Ok(ExpressionValue::option_none_with_type(
                        context.runtime().type_to_arrow_datatype(&args[0]),
                    ))
                } else {
                    let value = self.untyped(context).await;
                    Ok(ExpressionValue::string(value))
                }
            }
            Type::Named(_) => Ok(ExpressionValue::unit()),
            Type::Generic(_) => Ok(ExpressionValue::unit()),
        }
    }

    async fn select(
        &self,
        _context: &Context,
        _options: &[ExpressionValue],
    ) -> Result<usize, String> {
        Ok(0)
    }

    async fn fill_parameter(
        &self,
        context: &Context,
        _param_name: &str,
        param_type: &Type,
    ) -> Result<ExpressionValue, String> {
        match param_type {
            _ if param_type.is_string() => {
                let value = self.untyped(context).await;
                Ok(ExpressionValue::string(value))
            }
            _ if param_type.is_boolean() => Ok(ExpressionValue::boolean(true)),
            _ if param_type.is_int() => Ok(ExpressionValue::integer(0)),
            Type::Parameterized(n, args) => {
                if n.last_name() == "Option" {
                    Ok(ExpressionValue::option_none_with_type(
                        context.runtime().type_to_arrow_datatype(&args[0]),
                    ))
                } else {
                    let value = self.untyped(context).await;
                    Ok(ExpressionValue::string(value))
                }
            }
            Type::Named(_) | Type::Generic(_) => Ok(ExpressionValue::unit()),
        }
    }
}
