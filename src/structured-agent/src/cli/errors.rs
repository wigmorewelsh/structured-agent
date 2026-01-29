use std::error::Error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum CliError {
    IoError(io::Error),
    McpError(String),
    RuntimeError(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::IoError(e) => write!(f, "File I/O error: {}", e),
            CliError::McpError(e) => write!(f, "MCP connection error: {}", e),
            CliError::RuntimeError(e) => {
                if e.contains("ExecutionError") && e.contains("Parse error at line") {
                    if let Some(start) = e.find("Parse error at line") {
                        let error_part = &e[start..];
                        if let Some(end) = error_part.find("\")") {
                            write!(f, "{}", &error_part[..end])
                        } else {
                            write!(f, "{}", error_part)
                        }
                    } else {
                        write!(f, "Parse error: {}", e)
                    }
                } else {
                    write!(f, "Execution error: {}", e)
                }
            }
        }
    }
}

impl Error for CliError {}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        CliError::IoError(err)
    }
}
