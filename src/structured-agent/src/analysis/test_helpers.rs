use crate::ast::Module;
use crate::compiler::{CodespanParser, CompilationUnit};
use crate::diagnostics::DiagnosticManager;

pub fn parse_code(code: &str) -> Module {
    let unit = CompilationUnit::from_string(code.to_string());
    let manager = DiagnosticManager::new();
    let parser = CodespanParser::new();
    parser.parse(&unit, 0, manager.reporter()).unwrap()
}
