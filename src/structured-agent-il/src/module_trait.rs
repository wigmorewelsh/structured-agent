use crate::NativeFunctionDef;
use structured_agent_runtime::{Parameter, Type};

pub struct NativeTraitFnDecl {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
}

pub struct NativeTraitDecl {
    pub name: String,
    pub functions: Vec<NativeTraitFnDecl>,
}

pub struct NativeImplDecl {
    pub type_name: String,
    pub type_params: Vec<String>,
    pub trait_name: Option<String>,
    pub functions: Vec<NativeFunctionDef>,
}

pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn native_functions(&self) -> Vec<NativeFunctionDef>;
    fn native_traits(&self) -> Vec<NativeTraitDecl> {
        vec![]
    }
    fn native_impls(&self) -> Vec<NativeImplDecl> {
        vec![]
    }
}
