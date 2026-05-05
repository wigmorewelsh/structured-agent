use std::sync::Arc;

use nonempty::NonEmpty;
use structured_agent_runtime::symbols::TypeAnnotation;

use crate::types::{FileId, Span, Spanned};

#[derive(Debug, Clone)]
pub struct ParsedModule {
    pub name: NonEmpty<String>,
    pub module: Module,
    pub is_entry: bool,
    pub file_id: FileId,
    pub is_inline: bool,
}
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub definitions: Vec<Definition>,
    pub span: Span,
    pub file_id: FileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<Type>,
}

impl TypeParam {
    pub fn unbounded(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bounds: vec![],
        }
    }
}

impl From<String> for TypeParam {
    fn from(name: String) -> Self {
        Self {
            name,
            bounds: vec![],
        }
    }
}

impl From<&str> for TypeParam {
    fn from(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bounds: vec![],
        }
    }
}

impl PartialEq<&str> for TypeParam {
    fn eq(&self, other: &&str) -> bool {
        self.bounds.is_empty() && self.name == *other
    }
}

impl PartialEq<String> for TypeParam {
    fn eq(&self, other: &String) -> bool {
        self.bounds.is_empty() && self.name == *other
    }
}

impl fmt::Display for TypeParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.bounds.is_empty() {
            let bounds: Vec<String> = self.bounds.iter().map(|b| b.to_string()).collect();
            write!(f, ": {}", bounds.join(" + "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigFunction {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AstTraitImpl {
    pub type_name: String,
    pub trait_name: Option<String>,
    pub functions: Vec<Arc<Function>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AstSignature {
    pub name: String,
    pub functions: Vec<SigFunction>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AstTrait {
    pub name: String,
    pub functions: Vec<SigFunction>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathArg {
    Positional(AstPath),
    Named { name: String, path: AstPath },
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: String,
    pub params: Vec<PathArg>,
    pub span: Span,
}

impl PartialEq for PathSegment {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.params == other.params
    }
}

impl Eq for PathSegment {}

impl PathSegment {
    pub fn simple(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            params: vec![],
            span: Span::dummy(),
        }
    }
}

pub type AstPath = NonEmpty<PathSegment>;

#[derive(Debug, Clone, PartialEq)]
pub struct Use {
    pub path: AstPath,
    pub alias: Option<String>,
    pub is_pub: bool,
    pub rooted: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    Function(Arc<Function>),
    ExternalFunction(Arc<ExternalFunction>),
    Struct(Arc<StructDefinition>),
    Use(Arc<Use>),
    ModuleHeader {
        name: String,
        span: Span,
    },
    Signature(Arc<AstSignature>),
    Trait(Arc<AstTrait>),
    TraitImpl(Arc<AstTraitImpl>),
    InlineModule {
        name: String,
        definitions: Vec<Definition>,
        span: Span,
    },
}

impl Spanned for Definition {
    fn span(&self) -> Span {
        match self {
            Definition::Function(f) => f.span,
            Definition::ExternalFunction(f) => f.span,
            Definition::Struct(s) => s.span,
            Definition::Use(u) => u.span,
            Definition::ModuleHeader { span, .. } => *span,
            Definition::Signature(s) => s.span,
            Definition::Trait(s) => s.span,
            Definition::TraitImpl(t) => t.span,
            Definition::InlineModule { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDefinition {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub field_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub body: FunctionBody,
    pub documentation: Option<String>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalFunction {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub path: AstPath,
    pub args: Vec<Type>,
}

impl Type {
    pub fn simple(name: impl Into<std::string::String>) -> Self {
        Self {
            path: NonEmpty::new(PathSegment::simple(name)),
            args: vec![],
        }
    }

    pub fn name(&self) -> &str {
        &self.path.first().name
    }

    pub fn parameterized(name: impl Into<std::string::String>, args: Vec<Type>) -> Self {
        Self {
            path: NonEmpty::new(PathSegment::simple(name)),
            args,
        }
    }
}

impl Spanned for Type {
    fn span(&self) -> Span {
        Span::dummy()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionBody {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Injection(Expression),
    Assignment {
        variable: String,
        expression: Expression,
        span: Span,
    },
    VariableAssignment {
        variable: String,
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
    Return(Expression),
    Yield {
        span: Span,
    },
}

impl Spanned for Statement {
    fn span(&self) -> Span {
        match self {
            Statement::Injection(expr) => expr.span(),
            Statement::Assignment { span, .. } => *span,
            Statement::VariableAssignment { span, .. } => *span,
            Statement::ExpressionStatement(expr) => expr.span(),
            Statement::If { span, .. } => *span,
            Statement::While { span, .. } => *span,
            Statement::Return(expr) => expr.span(),
            Statement::Yield { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectExpression {
    pub clauses: Vec<SelectClause>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectClause {
    pub expression_to_run: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Call {
        function: String,
        type_args: Vec<Type>,
        arguments: Vec<Expression>,
        span: Span,
    },
    Variable {
        name: String,
        span: Span,
    },
    StructLiteral {
        struct_name: String,
        fields: Vec<(String, Expression)>,
        span: Span,
    },
    FieldAccess {
        base: Box<Expression>,
        field: String,
        span: Span,
    },
    MethodCall {
        receiver: Box<Expression>,
        method: String,
        args: Vec<Expression>,
        span: Span,
    },
    StringLiteral {
        value: String,
        span: Span,
    },
    BooleanLiteral {
        value: bool,
        span: Span,
    },
    IntLiteral {
        value: i64,
        span: Span,
    },
    ListLiteral {
        elements: Vec<Expression>,
        span: Span,
    },
    Placeholder {
        span: Span,
    },
    UnitLiteral {
        span: Span,
    },
    Select(SelectExpression),
    IfElse {
        condition: Box<Expression>,
        then_expr: Box<Expression>,
        else_expr: Box<Expression>,
        span: Span,
    },
    Spawn {
        type_arg: Type,
        key: Box<Expression>,
        span: Span,
    },
}

impl Spanned for Expression {
    fn span(&self) -> Span {
        match self {
            Expression::Call { span, .. } => *span,
            Expression::Variable { span, .. } => *span,
            Expression::StringLiteral { span, .. } => *span,
            Expression::BooleanLiteral { span, .. } => *span,
            Expression::IntLiteral { span, .. } => *span,
            Expression::ListLiteral { span, .. } => *span,
            Expression::Placeholder { span } => *span,
            Expression::UnitLiteral { span } => *span,
            Expression::Select(select) => select.span,
            Expression::IfElse { span, .. } => *span,
            Expression::StructLiteral { span, .. } => *span,
            Expression::FieldAccess { span, .. } => *span,
            Expression::MethodCall { span, .. } => *span,
            Expression::Spawn { span, .. } => *span,
        }
    }
}

impl Spanned for SelectExpression {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for SelectClause {
    fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.params.is_empty() {
            write!(f, "(")?;
            for (i, p) in self.params.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", p)?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

impl fmt::Display for PathArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathArg::Positional(path) => {
                let s: Vec<String> = path.iter().map(|seg| seg.to_string()).collect();
                write!(f, "{}", s.join("::"))
            }
            PathArg::Named { name, path } => {
                let s: Vec<String> = path.iter().map(|seg| seg.to_string()).collect();
                write!(f, "{}: {}", name, s.join("::"))
            }
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path_str: Vec<String> = self.path.iter().map(|seg| seg.to_string()).collect();
        if self.args.is_empty() {
            write!(f, "{}", path_str.join("::"))
        } else {
            write!(
                f,
                "{}<{}>",
                path_str.join("::"),
                self.args
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(doc) = &self.documentation {
            for line in doc.lines() {
                writeln!(f, "## {}", line)?;
            }
        }
        if self.is_pub {
            write!(f, "pub ")?;
        }
        write!(f, "fn {}(", self.name)?;
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", param.name, param.param_type)?;
        }
        write!(f, "): {}", self.return_type)?;
        Ok(())
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Statement::Injection(expr) => write!(f, "{}!", expr),
            Statement::Assignment {
                variable,
                expression,
                ..
            } => {
                write!(f, "let {} = {}", variable, expression)
            }
            Statement::VariableAssignment {
                variable,
                expression,
                ..
            } => {
                write!(f, "{} = {}", variable, expression)
            }
            Statement::ExpressionStatement(expr) => write!(f, "{}", expr),
            Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                writeln!(f, "if {} {{", condition)?;
                for stmt in body {
                    writeln!(f, "    {}", stmt)?;
                }
                if let Some(else_stmts) = else_body {
                    writeln!(f, "}} else {{")?;
                    for stmt in else_stmts {
                        writeln!(f, "    {}", stmt)?;
                    }
                    write!(f, "}}")
                } else {
                    write!(f, "}}")
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                writeln!(f, "while {} {{", condition)?;
                for stmt in body {
                    writeln!(f, "    {}", stmt)?;
                }
                write!(f, "}}")
            }
            Statement::Return(expr) => write!(f, "return {}", expr),
            Statement::Yield { .. } => write!(f, "yield"),
        }
    }
}

impl fmt::Display for SelectExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "select {{")?;
        for clause in &self.clauses {
            writeln!(f, "    {},", clause.expression_to_run)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, definition) in self.definitions.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{}", definition)?;
        }
        Ok(())
    }
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Definition::Function(func) => write!(f, "{}", func),
            Definition::ExternalFunction(ext_func) => write!(f, "{}", ext_func),
            Definition::Struct(s) => {
                if s.type_params.is_empty() {
                    write!(f, "struct {} {{", s.name)?;
                } else {
                    let params: Vec<&str> =
                        s.type_params.iter().map(|tp| tp.name.as_str()).collect();
                    write!(f, "struct {}<{}> {{", s.name, params.join(", "))?;
                }
                for field in &s.fields {
                    write!(f, "\n    {}: {},", field.name, field.field_type)?;
                }
                write!(f, "\n}}")
            }
            Definition::Use(u) => {
                let path = &u.path;
                let alias = &u.alias;
                let is_pub = u.is_pub;
                if is_pub {
                    write!(f, "pub ")?;
                }
                write!(f, "use ")?;
                if u.rooted {
                    write!(f, "::")?;
                }
                for (i, seg) in path.iter().enumerate() {
                    if i > 0 {
                        write!(f, "::")?;
                    }
                    write!(f, "{}", seg)?;
                }
                if let Some(a) = alias {
                    write!(f, " as {}", a)?;
                }
                Ok(())
            }
            Definition::ModuleHeader { name, .. } => {
                write!(f, "mod {}", name)
            }
            Definition::Signature(s) => {
                writeln!(f, "sig {} {{", s.name)?;
                for sig_fn in &s.functions {
                    write!(f, "    fn {}(", sig_fn.name)?;
                    for (i, param) in sig_fn.parameters.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}: {}", param.name, param.param_type)?;
                    }
                    writeln!(f, "): {}", sig_fn.return_type)?;
                }
                write!(f, "}}")
            }
            Definition::Trait(s) => {
                write!(f, "trait {}", s.name)?;
                write!(f, " {{")?;
                for func in &s.functions {
                    write!(f, "\n    fn {}(", func.name)?;
                    for (i, p) in func.parameters.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}: {}", p.name, p.param_type)?;
                    }
                    write!(f, "): {}", func.return_type)?;
                }
                write!(f, "\n}}")
            }
            Definition::TraitImpl(t) => {
                match &t.trait_name {
                    Some(tn) => write!(f, "impl {}: {}", t.type_name, tn)?,
                    None => write!(f, "impl {}", t.type_name)?,
                }
                write!(f, " {{")?;
                for func in &t.functions {
                    write!(f, "\n    {}", func)?;
                }
                write!(f, "\n}}")
            }
            Definition::InlineModule {
                name, definitions, ..
            } => {
                writeln!(f, "mod {} {{", name)?;
                for def in definitions {
                    writeln!(f, "    {}", def)?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl fmt::Display for ExternalFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_pub {
            write!(f, "pub ")?;
        }
        write!(f, "extern fn {}", self.name)?;
        if !self.type_params.is_empty() {
            write!(f, "<")?;
            for (i, tp) in self.type_params.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", tp)?;
            }
            write!(f, ">")?;
        }
        write!(f, "(")?;
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", param.name, param.param_type)?;
        }
        write!(f, "): {}", self.return_type)
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Call {
                function,
                type_args,
                arguments,
                ..
            } => {
                write!(f, "{}", function)?;
                if !type_args.is_empty() {
                    write!(f, "<")?;
                    for (i, ta) in type_args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", ta)?;
                    }
                    write!(f, ">")?;
                }
                write!(f, "(")?;
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expression::Variable { name, .. } => write!(f, "{}", name),
            Expression::StringLiteral { value, .. } => write!(f, "\"{}\"", value),
            Expression::BooleanLiteral { value, .. } => write!(f, "{}", value),
            Expression::IntLiteral { value, .. } => write!(f, "{}", value),
            Expression::ListLiteral { elements, .. } => {
                write!(f, "[")?;
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            }
            Expression::Placeholder { .. } => write!(f, "_"),
            Expression::UnitLiteral { .. } => write!(f, "()"),
            Expression::Select(select) => write!(f, "{}", select),
            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => write!(
                f,
                "if {} {{ {} }} else {{ {} }}",
                condition, then_expr, else_expr
            ),
            Expression::StructLiteral {
                struct_name,
                fields,
                ..
            } => {
                write!(f, "{} {{", struct_name)?;
                for (i, (name, expr)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, " {}: {}", name, expr)?;
                }
                write!(f, " }}")
            }
            Expression::FieldAccess { base, field, .. } => write!(f, "{}.{}", base, field),
            Expression::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                write!(f, "{}.{}(", receiver, method)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expression::Spawn { type_arg, key, .. } => {
                write!(f, "spawn<{}>({})", type_arg, key)
            }
        }
    }
}

impl TypeAnnotation for Type {}
