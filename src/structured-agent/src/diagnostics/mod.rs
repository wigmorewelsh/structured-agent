pub mod reporter;

pub use reporter::DiagnosticReporter;

use crate::types::{FileId, SourceFiles};
use codespan_reporting::diagnostic::Diagnostic;

pub struct DiagnosticManager {
    files: SourceFiles,
    reporter: DiagnosticReporter,
}

impl DiagnosticManager {
    pub fn new() -> Self {
        let files = SourceFiles::new();
        let reporter = DiagnosticReporter::new(files.clone());
        Self { files, reporter }
    }

    pub fn add_file(&mut self, name: String, source: String) -> FileId {
        self.files.add(name, source)
    }

    pub fn files(&self) -> &SourceFiles {
        &self.files
    }

    pub fn reporter(&self) -> &DiagnosticReporter {
        &self.reporter
    }
}

impl Default for DiagnosticManager {
    fn default() -> Self {
        Self::new()
    }
}
