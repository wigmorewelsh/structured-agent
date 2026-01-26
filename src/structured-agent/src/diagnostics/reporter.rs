use crate::typecheck::TypeError;
use crate::types::{FileId, SourceFiles};
use codespan_reporting::diagnostic::Diagnostic;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::term::{self, Config};

#[derive(Clone)]
pub struct DiagnosticReporter {
    files: SourceFiles,
    config: Config,
}

impl DiagnosticReporter {
    pub fn new(files: SourceFiles) -> Self {
        Self {
            files,
            config: Config::default(),
        }
    }

    pub fn emit_type_error(&self, error: &TypeError) -> Result<(), Box<dyn std::error::Error>> {
        let diagnostic = error.to_diagnostic();
        self.emit_diagnostic(&diagnostic)
    }

    pub fn emit_parse_error(
        &self,
        file_id: FileId,
        error: &str,
        span: Option<(usize, usize)>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let diagnostic = if let Some((start, end)) = span {
            Diagnostic::error()
                .with_message("parse error")
                .with_labels(vec![
                    codespan_reporting::diagnostic::Label::primary(file_id, start..end)
                        .with_message(error),
                ])
        } else {
            Diagnostic::error().with_message(format!("parse error: {}", error))
        };

        self.emit_diagnostic(&diagnostic)
    }

    pub fn emit_parse_error_with_span(
        &self,
        file_id: FileId,
        error: &str,
        span: crate::types::Span,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let diagnostic = Diagnostic::error()
            .with_message("parse error")
            .with_labels(vec![
                codespan_reporting::diagnostic::Label::primary(file_id, span.to_byte_range())
                    .with_message(error),
            ]);

        self.emit_diagnostic(&diagnostic)
    }

    pub fn emit_diagnostic(
        &self,
        diagnostic: &Diagnostic<FileId>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let writer = StandardStream::stderr(ColorChoice::Auto);
        let files = self.files.files();
        term::emit(
            &mut writer.lock(),
            &self.config,
            &*files.borrow(),
            diagnostic,
        )
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    pub fn files(&self) -> &SourceFiles {
        &self.files
    }

    pub fn files_mut(&mut self) -> &mut SourceFiles {
        &mut self.files
    }
}

impl Default for DiagnosticReporter {
    fn default() -> Self {
        Self::new(SourceFiles::new())
    }
}
