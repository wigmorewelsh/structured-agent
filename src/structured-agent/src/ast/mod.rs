use crate::types::{FileId, Span, Spanned};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub definitions: Vec<Definition>,
    pub span: Span,
    pub file_id: FileId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    Function(Function),
    ExternalFunction(ExternalFunction),
}

impl Spanned for Definition {
    fn span(&self) -> Span {
        match self {
            Definition::Function(f) => f.span,
            Definition::ExternalFunction(f) => f.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub body: FunctionBody,
    pub documentation: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalFunction {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    Boolean,
    String,
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
    pub result_variable: String,
    pub expression_next: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Call {
        function: String,
        arguments: Vec<Expression>,
        span: Span,
    },
    Variable {
        name: String,
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
    Placeholder {
        span: Span,
    },
    Select(SelectExpression),
    IfElse {
        condition: Box<Expression>,
        then_expr: Box<Expression>,
        else_expr: Box<Expression>,
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
            Expression::Placeholder { span } => *span,
            Expression::Select(select) => select.span,
            Expression::IfElse { span, .. } => *span,
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

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Unit => write!(f, "()"),
            Type::Boolean => write!(f, "Boolean"),
            Type::String => write!(f, "String"),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(doc) = &self.documentation {
            for line in doc.lines() {
                writeln!(f, "# {}", line)?;
            }
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
        }
    }
}

impl fmt::Display for SelectExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "select {{")?;
        for clause in &self.clauses {
            writeln!(
                f,
                "    {} as {} => {},",
                clause.expression_to_run, clause.result_variable, clause.expression_next
            )?;
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
        }
    }
}

impl fmt::Display for ExternalFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "extern fn {}(", self.name)?;
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
                arguments,
                ..
            } => {
                write!(f, "{}", function)?;
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
            Expression::Placeholder { .. } => write!(f, "_"),
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
        }
    }
}
