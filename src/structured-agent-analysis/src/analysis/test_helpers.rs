use structured_agent_ast::ast::Module;
use structured_agent::compiler::{CodespanParser, CompilationUnit};
use structured_agent::diagnostics::DiagnosticManager;

pub fn parse_code(code: &str) -> Module {
    let unit = CompilationUnit::from_string(code.to_string());
    let manager = DiagnosticManager::new();
    let parser = CodespanParser::new();
    parser.parse(&unit, 0, manager.reporter()).unwrap()
}
