use crate::ast::Module;
use crate::diagnostics::{DiagnosticManager, DiagnosticReporter};
use crate::types::FileId;

use super::CodespanParser;
use super::CompilationUnit;
use super::discovery::DiscoveredFile;

#[derive(Debug)]
pub(crate) struct ParsedModule {
    pub(crate) name: String,
    pub(crate) module: Module,
    pub(crate) is_entry: bool,
    pub(crate) file_id: FileId,
}

pub(crate) fn parse_modules(
    files: &[DiscoveredFile],
    diagnostics: &mut DiagnosticManager,
    parser: &CodespanParser,
) -> Result<Vec<ParsedModule>, String> {
    files
        .iter()
        .map(|file| parse_one(file, diagnostics, parser))
        .collect()
}

fn parse_one(
    file: &DiscoveredFile,
    diagnostics: &mut DiagnosticManager,
    parser: &CodespanParser,
) -> Result<ParsedModule, String> {
    let file_id = diagnostics.add_file(file.path.clone(), file.source.clone());
    let unit = CompilationUnit::from_file(file.path.clone(), file.source.clone());
    let reporter: DiagnosticReporter = diagnostics.reporter().clone();
    let module = parser.parse(&unit, file_id, &reporter)?;

    Ok(ParsedModule {
        name: file.name.clone(),
        module,
        is_entry: file.is_entry,
        file_id,
    })
}
