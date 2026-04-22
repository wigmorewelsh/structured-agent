use crate::ast::{
    AstSignature, AstTrait, AstTraitImpl, Function, Module, Parameter, StructDefinition,
    Type as AstType, TypeParam,
};
use crate::typed_ast;
use crate::types::{FileId, Span};
use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    AstRef, BodyRef, DefinitionPath, NoAst, References, SourceRef, TypeAnnotation, WitnessRef,
};

pub type ModuleVisibility = HashMap<String, bool>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionKind {
    Bytecode,
    External,
}

#[derive(Debug, Clone)]
pub struct SourceLocation(pub FileId, pub Span);
pub struct NoBody;
#[derive(Debug, Clone, Default)]
pub struct NoWitness;

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

impl SourceRef for SourceLocation {}
impl AstRef for CheckerAstRef {}
impl BodyRef for NoBody {}
impl WitnessRef for NoWitness {}
impl TypeAnnotation for AstType {}

#[derive(Clone)]
pub struct CheckerRefs;

impl References for CheckerRefs {
    type Source = SourceLocation;
    type Ast = CheckerAstRef;
    type Body = NoBody;
    type Witness = NoWitness;
    type TypeAnnotation = AstType;
}

#[derive(Clone)]
pub struct PrimitiveRefs;

impl References for PrimitiveRefs {
    type Source = SourceLocation;
    type Ast = NoAst;
    type Body = NoBody;
    type Witness = NoWitness;
    type TypeAnnotation = DefinitionPath;
}

#[derive(Clone)]
pub enum TypedCheckerAstRef {
    Function(Arc<typed_ast::Function>, FunctionKind),
    ImplFunction(Arc<typed_ast::Function>, String, FunctionKind),
    Other(CheckerAstRef),
    NoAst,
}

impl AstRef for TypedCheckerAstRef {}

#[derive(Clone)]
pub struct TypedRefs;

impl References for TypedRefs {
    type Source = SourceLocation;
    type Ast = TypedCheckerAstRef;
    type Body = NoBody;
    type Witness = NoWitness;
    type TypeAnnotation = DefinitionPath;
}
