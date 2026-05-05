#![allow(dead_code)]

use structured_agent_runtime::DefinitionPath;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindingId(pub u32);

pub mod refs;
pub use refs::{NoBody, NoWitness, SourceLocation, TypedCheckerAstRef, TypedRefs};

use structured_agent_ast::ast::{ExternalFunction, PathSegment, StructDefinition};
use structured_agent_ast::types::{FileId, Span};

use structured_agent_runtime::Type;
use structured_agent_runtime::symbols::FunctionKind;

#[derive(Clone, Debug)]
pub struct Module {
    pub definitions: Vec<Definition>,
    pub span: Span,
    pub file_id: FileId,
}

#[derive(Clone, Debug)]
pub enum Definition {
    Function(Function),
    ExternalFunction(ExternalFunction),
    Struct(StructDefinition),
    Use {
        path: Vec<PathSegment>,
        name: String,
        alias: Option<String>,
        is_pub: bool,
        span: Span,
    },

    Trait {
        name: String,
        functions: Vec<structured_agent_ast::ast::SigFunction>,
        span: Span,
    },
    TraitImpl {
        type_name: String,
        trait_name: Option<String>,
        functions: Vec<Function>,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Type,
    pub binding_id: BindingId,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub body: FunctionBody,
    pub documentation: Option<String>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct FunctionBody {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Statement {
    Injection(Expression),
    Assignment {
        variable: String,
        binding_id: BindingId,
        expression: Expression,
        span: Span,
    },
    VariableAssignment {
        variable: String,
        binding_id: BindingId,
        expression: Expression,
        span: Span,
    },
    ExpressionStatement(Expression),
    If {
        condition: Expression,
        body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
        span: Span,
    },
    While {
        condition: Expression,
        body: Vec<Statement>,
        span: Span,
    },
    ForIn {
        variable: String,
        binding_id: BindingId,
        iterable: Expression,
        move_next_fn: DefinitionPath,
        current_fn: DefinitionPath,
        body: Vec<Statement>,
        span: Span,
    },
    Return(Expression),
    Yield {
        span: Span,
    },
}

#[derive(Clone, Debug)]
pub struct SelectExpression {
    pub clauses: Vec<SelectClause>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct SelectClause {
    pub expression_to_run: Expression,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum MethodBinding {
    Early(DefinitionPath),
    Late(BindingId, DefinitionPath),
}

#[derive(Clone, Debug)]
pub enum Expression {
    Call {
        function: String,
        binding: MethodBinding,
        kind: FunctionKind,
        arguments: Vec<Expression>,
        target: Option<Box<Expression>>,
        ty: Type,
        span: Span,
    },
    Variable {
        name: String,
        binding_id: BindingId,
        ty: Type,
        span: Span,
    },
    StructLiteral {
        struct_name: String,
        fields: Vec<(String, Expression)>,
        ty: Type,
        span: Span,
    },
    FieldAccess {
        base: Box<Expression>,
        field: String,
        ty: Type,
        span: Span,
    },
    StringLiteral {
        value: String,
        ty: Type,
        span: Span,
    },
    BooleanLiteral {
        value: bool,
        ty: Type,
        span: Span,
    },
    IntLiteral {
        value: i64,
        ty: Type,
        span: Span,
    },
    ListLiteral {
        elements: Vec<Expression>,
        ty: Type,
        span: Span,
    },
    Placeholder {
        ty: Type,
        span: Span,
    },
    UnitLiteral {
        ty: Type,
        span: Span,
    },
    TypeLiteral {
        ty: Type,
        span: Span,
    },
    ModuleInstance {
        path: DefinitionPath,
        ty: Type,
        span: Span,
    },
    Select(SelectExpression, Type),
    IfElse {
        condition: Box<Expression>,
        then_expr: Box<Expression>,
        else_expr: Box<Expression>,
        ty: Type,
        span: Span,
    },
    Spawn {
        key: Box<Expression>,
        ty: Type,
        span: Span,
    },
}

impl Expression {
    pub fn ty(&self) -> &Type {
        match self {
            Expression::Call { ty, .. } => ty,
            Expression::Variable { ty, .. } => ty,
            Expression::StructLiteral { ty, .. } => ty,
            Expression::FieldAccess { ty, .. } => ty,
            Expression::StringLiteral { ty, .. } => ty,
            Expression::BooleanLiteral { ty, .. } => ty,
            Expression::IntLiteral { ty, .. } => ty,
            Expression::ListLiteral { ty, .. } => ty,
            Expression::Placeholder { ty, .. } => ty,
            Expression::UnitLiteral { ty, .. } => ty,
            Expression::TypeLiteral { ty, .. } => ty,
            Expression::ModuleInstance { ty, .. } => ty,
            Expression::Select(_, ty) => ty,
            Expression::IfElse { ty, .. } => ty,
            Expression::Spawn { ty, .. } => ty,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Expression::Call { span, .. } => *span,
            Expression::Variable { span, .. } => *span,
            Expression::StructLiteral { span, .. } => *span,
            Expression::FieldAccess { span, .. } => *span,
            Expression::StringLiteral { span, .. } => *span,
            Expression::BooleanLiteral { span, .. } => *span,
            Expression::IntLiteral { span, .. } => *span,
            Expression::ListLiteral { span, .. } => *span,
            Expression::Placeholder { span, .. } => *span,
            Expression::UnitLiteral { span, .. } => *span,
            Expression::TypeLiteral { span, .. } => *span,
            Expression::ModuleInstance { span, .. } => *span,
            Expression::Select(s, _) => s.span,
            Expression::IfElse { span, .. } => *span,
            Expression::Spawn { span, .. } => *span,
        }
    }
}
