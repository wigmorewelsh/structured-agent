use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub body: FunctionBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(String),
    Unit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionBody {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Injection(Expression),
    Assignment {
        variable: String,
        expression: Expression,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Call {
        target: String,
        function: String,
        arguments: Vec<Expression>,
        is_method: bool,
    },
    Variable(String),
    StringLiteral(String),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Named(name) => write!(f, "{}", name),
            Type::Unit => write!(f, "()"),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn {}(", self.name)?;
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", param.name, param.param_type)?;
        }
        write!(f, ") -> {}", self.return_type)?;
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
            } => {
                write!(f, "let {} = {}", variable, expression)
            }
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Call {
                target,
                function,
                arguments,
                is_method,
            } => {
                if *is_method {
                    write!(f, "{}.{}", target, function)?;
                } else {
                    write!(f, "{}::{}", target, function)?;
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
            Expression::Variable(name) => write!(f, "{}", name),
            Expression::StringLiteral(content) => write!(f, "\"{}\"", content),
        }
    }
}
