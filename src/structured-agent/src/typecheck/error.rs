use crate::types::{FileId, Span};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    UnknownVariable {
        name: String,
        span: Span,
        file_id: FileId,
    },
    UnknownFunction {
        name: String,
        span: Span,
        file_id: FileId,
    },
    TypeMismatch {
        expected: String,
        found: String,
        span: Span,
        file_id: FileId,
    },
    VariableTypeMismatch {
        variable: String,
        expected: String,
        found: String,
        span: Span,
        declaration_span: Span,
        file_id: FileId,
    },
    ArgumentCountMismatch {
        function: String,
        expected: usize,
        found: usize,
        span: Span,
        file_id: FileId,
    },
    ArgumentTypeMismatch {
        function: String,
        parameter: String,
        expected: String,
        found: String,
        span: Span,
        file_id: FileId,
    },
    ReturnTypeMismatch {
        function: String,
        expected: String,
        found: String,
        span: Span,
        file_id: FileId,
    },
    SelectBranchTypeMismatch {
        expected: String,
        found: String,
        branch_index: usize,
        span: Span,
        first_branch_span: Span,
        file_id: FileId,
    },
    UnsupportedType {
        type_name: String,
        span: Span,
        file_id: FileId,
    },
}

impl TypeError {
    pub fn span(&self) -> Span {
        match self {
            TypeError::UnknownVariable { span, .. } => *span,
            TypeError::UnknownFunction { span, .. } => *span,
            TypeError::TypeMismatch { span, .. } => *span,
            TypeError::VariableTypeMismatch { span, .. } => *span,
            TypeError::ArgumentCountMismatch { span, .. } => *span,
            TypeError::ArgumentTypeMismatch { span, .. } => *span,
            TypeError::ReturnTypeMismatch { span, .. } => *span,
            TypeError::SelectBranchTypeMismatch { span, .. } => *span,
            TypeError::UnsupportedType { span, .. } => *span,
        }
    }

    pub fn file_id(&self) -> FileId {
        match self {
            TypeError::UnknownVariable { file_id, .. } => *file_id,
            TypeError::UnknownFunction { file_id, .. } => *file_id,
            TypeError::TypeMismatch { file_id, .. } => *file_id,
            TypeError::VariableTypeMismatch { file_id, .. } => *file_id,
            TypeError::ArgumentCountMismatch { file_id, .. } => *file_id,
            TypeError::ArgumentTypeMismatch { file_id, .. } => *file_id,
            TypeError::ReturnTypeMismatch { file_id, .. } => *file_id,
            TypeError::SelectBranchTypeMismatch { file_id, .. } => *file_id,
            TypeError::UnsupportedType { file_id, .. } => *file_id,
        }
    }

    pub fn to_diagnostic(&self) -> codespan_reporting::diagnostic::Diagnostic<FileId> {
        use codespan_reporting::diagnostic::{Diagnostic, Label};

        match self {
            TypeError::UnknownVariable {
                name,
                span,
                file_id,
            } => Diagnostic::error()
                .with_message(format!("unknown variable `{}`", name))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("not found in this scope"),
                ]),
            TypeError::UnknownFunction {
                name,
                span,
                file_id,
            } => Diagnostic::error()
                .with_message(format!("unknown function `{}`", name))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("function not declared"),
                ]),
            TypeError::TypeMismatch {
                expected,
                found,
                span,
                file_id,
            } => Diagnostic::error()
                .with_message("type mismatch")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message(format!("expected `{}`, found `{}`", expected, found)),
                ]),
            TypeError::VariableTypeMismatch {
                variable,
                expected,
                found,
                span,
                declaration_span,
                file_id,
            } => Diagnostic::error()
                .with_message(format!(
                    "cannot assign `{}` to variable `{}`",
                    found, variable
                ))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message(format!("expected `{}`, found `{}`", expected, found)),
                    Label::secondary(*file_id, declaration_span.to_byte_range())
                        .with_message(format!("variable declared here with type `{}`", expected)),
                ]),
            TypeError::ArgumentCountMismatch {
                function: _,
                expected,
                found,
                span,
                file_id,
            } => Diagnostic::error()
                .with_message(format!(
                    "this function takes {} arguments but {} were supplied",
                    expected, found
                ))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message(format!("expected {} arguments", expected)),
                ]),
            TypeError::ArgumentTypeMismatch {
                function,
                parameter,
                expected,
                found,
                span,
                file_id,
            } => Diagnostic::error()
                .with_message("mismatched argument type")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message(format!("expected `{}`, found `{}`", expected, found)),
                ])
                .with_notes(vec![format!(
                    "in function `{}`, parameter `{}`",
                    function, parameter
                )]),
            TypeError::ReturnTypeMismatch {
                function,
                expected,
                found,
                span,
                file_id,
            } => Diagnostic::error()
                .with_message("mismatched return type")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message(format!("expected `{}`, found `{}`", expected, found)),
                ])
                .with_notes(vec![format!("in function `{}`", function)]),
            TypeError::SelectBranchTypeMismatch {
                expected,
                found,
                branch_index,
                span,
                first_branch_span,
                file_id,
            } => Diagnostic::error()
                .with_message("select branches have incompatible types")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message(format!("expected `{}`, found `{}`", expected, found)),
                    Label::secondary(*file_id, first_branch_span.to_byte_range())
                        .with_message(format!("first branch has type `{}`", expected)),
                ])
                .with_notes(vec![format!("in select branch {}", branch_index)]),
            TypeError::UnsupportedType {
                type_name,
                span,
                file_id,
            } => Diagnostic::error()
                .with_message(format!("unsupported type `{}`", type_name))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("type not supported"),
                ]),
        }
    }
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::UnknownVariable { name, .. } => {
                write!(f, "Unknown variable: {}", name)
            }
            TypeError::UnknownFunction { name, .. } => {
                write!(f, "Unknown function: {}", name)
            }
            TypeError::TypeMismatch {
                expected, found, ..
            } => {
                write!(f, "Type mismatch: expected {}, found {}", expected, found)
            }
            TypeError::VariableTypeMismatch {
                variable,
                expected,
                found,
                ..
            } => {
                write!(
                    f,
                    "Variable {} type mismatch: expected {}, found {}",
                    variable, expected, found
                )
            }
            TypeError::ArgumentCountMismatch {
                function,
                expected,
                found,
                ..
            } => {
                write!(
                    f,
                    "Function {} expects {} arguments, found {}",
                    function, expected, found
                )
            }
            TypeError::ArgumentTypeMismatch {
                function,
                parameter,
                expected,
                found,
                ..
            } => {
                write!(
                    f,
                    "Function {}, parameter {}: expected {}, found {}",
                    function, parameter, expected, found
                )
            }
            TypeError::ReturnTypeMismatch {
                function,
                expected,
                found,
                ..
            } => {
                write!(
                    f,
                    "Function {} return type mismatch: expected {}, found {}",
                    function, expected, found
                )
            }
            TypeError::SelectBranchTypeMismatch {
                expected,
                found,
                branch_index,
                ..
            } => {
                write!(
                    f,
                    "Select branch {} type mismatch: expected {}, found {}",
                    branch_index, expected, found
                )
            }
            TypeError::UnsupportedType { type_name, .. } => {
                write!(f, "Unsupported type: {}", type_name)
            }
        }
    }
}

impl std::error::Error for TypeError {}
