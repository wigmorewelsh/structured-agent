use std::collections::HashMap;
use structured_agent_ast::ast::Type as AstType;
use structured_agent_runtime::symbols::{DefinitionPath, NoAst, References};

pub use structured_agent_ast::CheckerAstRef;
pub use structured_agent_runtime::symbols::FunctionKind;
pub use structured_agent_typed_ast::{
    NoBody, NoWitness, SourceLocation, TypedCheckerAstRef, TypedRefs,
};

pub type ModuleVisibility = HashMap<String, bool>;

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
