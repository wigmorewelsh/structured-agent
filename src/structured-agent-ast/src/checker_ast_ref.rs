use crate::ast::{
    AstSignature, AstTrait, AstTraitImpl, Function, Module, Parameter, StructDefinition,
    Type as AstType, TypeParam,
};
use std::sync::Arc;
use structured_agent_runtime::symbols::{AstRef, FunctionKind};

#[derive(Clone)]
pub enum CheckerAstRef {
    Function(Arc<Function>, FunctionKind),
    ImplFunction(Arc<Function>, String, FunctionKind),
    ExternalFn {
        params: Vec<Parameter>,
        return_type: AstType,
        type_params: Vec<TypeParam>,
        kind: FunctionKind,
    },
    Struct(Arc<StructDefinition>),
    Signature(Arc<AstSignature>),
    Trait(Arc<AstTrait>),
    Impl(Arc<AstTraitImpl>),
    Module(Arc<Module>),
    Primitive,
}

impl AstRef for CheckerAstRef {}
