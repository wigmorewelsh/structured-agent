use crate::Function;
use std::sync::Arc;
use structured_agent_ast::CheckerAstRef;
use structured_agent_ast::types::{FileId, Span};
pub use structured_agent_runtime::symbols::FunctionKind;
use structured_agent_runtime::symbols::{
    AstRef, BodyRef, DefinitionPath, References, SourceRef, WitnessRef, WitnessTable,
};

#[derive(Debug, Clone)]
pub struct SourceLocation(pub FileId, pub Span);

#[derive(Debug, Clone, Default)]
pub struct NoWitness;

pub struct NoBody;

#[derive(Clone)]
pub enum TypedCheckerAstRef {
    Function(Arc<Function>, FunctionKind),
    ImplFunction(Arc<Function>, String, FunctionKind),
    Other(CheckerAstRef),
    NoAst,
}

impl SourceRef for SourceLocation {}
impl AstRef for TypedCheckerAstRef {}
impl BodyRef for NoBody {}
impl WitnessRef for NoWitness {}

#[derive(Clone)]
pub struct TypedRefs;

impl References for TypedRefs {
    type Source = SourceLocation;
    type Ast = TypedCheckerAstRef;
    type Body = NoBody;
    type Witness = WitnessTable;
    type TypeAnnotation = DefinitionPath;
}
