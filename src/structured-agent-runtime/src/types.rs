use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type FileId = usize;

pub type NativeFn = dyn Fn(
        Vec<ExpressionValue>,
        AgentHandle,
    ) -> Pin<Box<dyn Future<Output = Result<ExpressionValue, String>> + Send>>
    + Send
    + Sync;

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

pub trait Spanned {
    fn span(&self) -> Span;
}

pub struct NativeFnPtr(pub Arc<NativeFn>);

impl NativeFnPtr {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(
                Vec<ExpressionValue>,
                AgentHandle,
            ) -> Pin<Box<dyn Future<Output = Result<ExpressionValue, String>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self(Arc::new(f))
    }

    pub fn call(
        &self,
        args: Vec<ExpressionValue>,
        handle: AgentHandle,
    ) -> Pin<Box<dyn Future<Output = Result<ExpressionValue, String>> + Send>> {
        (self.0)(args, handle)
    }
}

impl Clone for NativeFnPtr {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for NativeFnPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native fn>")
    }
}

impl PartialEq for NativeFnPtr {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
use nonempty::NonEmpty;

use crate::actor::AgentHandle;
use crate::expression::ExpressionValue;
use crate::symbols::DefinitionPath;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(DefinitionPath),
    Parameterized(DefinitionPath, Vec<Type>),
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
    fn prelude(name: &str) -> DefinitionPath {
        DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
            name,
        )
    }

    pub fn string() -> Self {
        Self::Named(Self::prelude("String"))
    }

    pub fn unit() -> Self {
        Self::Named(Self::prelude("Unit"))
    }

    pub fn boolean() -> Self {
        Self::Named(Self::prelude("Boolean"))
    }

    pub fn int() -> Self {
        Self::Named(Self::prelude("Int"))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Type::Named(tn) if tn.last_name() == "String")
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, Type::Named(tn) if tn.last_name() == "Boolean")
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Type::Named(tn) if tn.last_name() == "Int")
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Type::Named(tn) if tn.last_name() == "Unit")
    }

    pub fn list(inner: Type) -> Self {
        Self::Parameterized(Self::prelude("List"), vec![inner])
    }

    pub fn option(inner: Type) -> Self {
        Self::Parameterized(Self::prelude("Option"), vec![inner])
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Type::Parameterized(n, _) if n.last_name() == "List")
    }

    pub fn is_option(&self) -> bool {
        matches!(self, Type::Parameterized(n, _) if n.last_name() == "Option")
    }

    pub fn actor_ref(inner: Type) -> Self {
        Self::Parameterized(Self::prelude("ActorRef"), vec![inner])
    }

    pub fn is_actor_ref(&self) -> bool {
        matches!(self, Type::Parameterized(n, _) if n.last_name() == "ActorRef")
    }

    pub fn actor_ref_inner(&self) -> Option<&Type> {
        if let Type::Parameterized(n, args) = self {
            if n.last_name() == "ActorRef" {
                return args.first();
            }
        }
        None
    }

    pub fn generic(name: impl Into<std::string::String>) -> Self {
        Self::Generic(name.into())
    }

    pub fn name(&self) -> String {
        match self {
            Type::Named(tn) => tn.last_name().to_string(),
            Type::Parameterized(type_name, args) => {
                let arg_names: Vec<String> = args.iter().map(|a| a.name()).collect();
                format!("{}<{}>", type_name.last_name(), arg_names.join(", "))
            }
            Type::Generic(name) => name.clone(),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Named(tn) => write!(f, "{}", tn.last_name()),
            Type::Parameterized(type_name, args) => {
                let arg_strs: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(f, "{}<{}>", type_name.last_name(), arg_strs.join(", "))
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_fn_ptr_clone_is_equal() {
        let ptr = NativeFnPtr::new(|_, _| Box::pin(async { Ok(ExpressionValue::unit()) }));
        let cloned = ptr.clone();
        assert_eq!(ptr, cloned);
    }

    #[test]
    fn native_fn_ptr_debug_format() {
        let ptr = NativeFnPtr::new(|_, _| Box::pin(async { Ok(ExpressionValue::unit()) }));
        assert_eq!(format!("{:?}", ptr), "<native fn>");
    }

    #[test]
    fn native_fn_ptr_different_instances_are_not_equal() {
        let a = NativeFnPtr::new(|_, _| Box::pin(async { Ok(ExpressionValue::unit()) }));
        let b = NativeFnPtr::new(|_, _| Box::pin(async { Ok(ExpressionValue::unit()) }));
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn native_fn_ptr_call_invokes_closure() {
        let ptr = NativeFnPtr::new(|_, _| Box::pin(async { Ok(ExpressionValue::string("hello")) }));
        let handle = AgentHandle::detached();
        let result = ptr.call(vec![], handle).await.unwrap();
        assert_eq!(result, ExpressionValue::string("hello"));
    }
}
