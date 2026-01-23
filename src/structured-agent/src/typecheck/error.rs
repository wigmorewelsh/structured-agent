use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    UnknownVariable(String),
    UnknownFunction(String),
    TypeMismatch {
        expected: String,
        found: String,
        location: String,
    },
    ArgumentCountMismatch {
        function: String,
        expected: usize,
        found: usize,
    },
    ArgumentTypeMismatch {
        function: String,
        parameter: String,
        expected: String,
        found: String,
    },
    ReturnTypeMismatch {
        function: String,
        expected: String,
        found: String,
    },
    SelectBranchTypeMismatch {
        expected: String,
        found: String,
        branch_index: usize,
    },
    UnsupportedType(String),
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::UnknownVariable(name) => {
                write!(f, "Unknown variable: {}", name)
            }
            TypeError::UnknownFunction(name) => {
                write!(f, "Unknown function: {}", name)
            }
            TypeError::TypeMismatch {
                expected,
                found,
                location,
            } => {
                write!(
                    f,
                    "Type mismatch at {}: expected {}, found {}",
                    location, expected, found
                )
            }
            TypeError::ArgumentCountMismatch {
                function,
                expected,
                found,
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
            } => {
                write!(
                    f,
                    "Select branch {} type mismatch: expected {}, found {}",
                    branch_index, expected, found
                )
            }
            TypeError::UnsupportedType(type_name) => {
                write!(f, "Unsupported type: {}", type_name)
            }
        }
    }
}

impl std::error::Error for TypeError {}
