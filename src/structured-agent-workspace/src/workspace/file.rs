use crate::anchor::{self, PatchError};
use crate::parser::{LanguageType, ParsedSource};
use rmcp::ErrorData as McpError;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error)]
enum WorkspaceError {
    #[error("path '{0}' escapes workspace root")]
    PathTraversal(String),
    #[error("file '{0}' does not exist")]
    FileNotFound(String),
    #[error("'{0}' is not a file")]
    NotAFile(String),
    #[error("unsupported file type '{0}'")]
    UnsupportedFileType(String),
    #[error("symbol '{symbol}' not found in '{path}'")]
    SymbolNotFound { path: String, symbol: String },
    #[error("anchor not found: {0}")]
    AnchorNotFound(String),
    #[error("ambiguous anchor: {0}")]
    AmbiguousAnchor(String),
    #[error("start anchor is after end anchor")]
    AnchorOrderInvalid,
    #[error("provide both start_anchor and end_anchor, or neither")]
    InconsistentAnchors,
    #[error("I/O error on '{path}': {error}")]
    Io { path: String, error: std::io::Error },
    #[error("parse error in '{path}': {error}")]
    Parse { path: String, error: String },
}

impl From<WorkspaceError> for McpError {
    fn from(e: WorkspaceError) -> Self {
        match e {
            WorkspaceError::Io { .. } | WorkspaceError::Parse { .. } => {
                McpError::internal_error(e.to_string(), None)
            }
            _ => McpError::invalid_params(e.to_string(), None),
        }
    }
}

impl From<PatchError> for WorkspaceError {
    fn from(e: PatchError) -> Self {
        match e {
            PatchError::AnchorNotFound(a) => WorkspaceError::AnchorNotFound(a),
            PatchError::AmbiguousAnchor(a) => WorkspaceError::AmbiguousAnchor(a),
            PatchError::AnchorOrderInvalid => WorkspaceError::AnchorOrderInvalid,
        }
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components: Vec<Component> = vec![];
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            c => components.push(c),
        }
    }
    components.iter().collect()
}

struct FileLocation {
    full_path: PathBuf,
    relative_path: String,
}

impl FileLocation {
    fn new(workspace_root: &Path, relative_path: &str) -> Self {
        let full_path = normalize_path(&workspace_root.join(relative_path));
        Self {
            full_path,
            relative_path: relative_path.to_string(),
        }
    }
}

impl AsRef<Path> for FileLocation {
    fn as_ref(&self) -> &Path {
        &self.full_path
    }
}

pub struct ReadResult {
    pub content: String,
    source: String,
    span: Span,
    file_path: PathBuf,
    relative_path: String,
}

enum Span {
    WholeFile,
    Bytes(Range<usize>),
}

impl ReadResult {
    fn overwrite(&self, text: &str) -> Result<(), WorkspaceError> {
        let new_source = match &self.span {
            Span::WholeFile => text.to_string(),
            Span::Bytes(range) => {
                let mut s = self.source.clone();
                s.replace_range(range.clone(), text);
                s
            }
        };
        std::fs::write(&self.file_path, new_source).map_err(|e| WorkspaceError::Io {
            path: self.relative_path.clone(),
            error: e,
        })
    }
}

enum LocationKind {
    WholeFile,
    Symbol(String),
    AnchorRange { start: String, end: String },
    SymbolWithAnchors { symbol: String, start: String, end: String },
}

pub struct ContentLocation {
    full_path: PathBuf,
    relative_path: String,
    kind: LocationKind,
}

impl ContentLocation {
    pub fn read(&self) -> Result<ReadResult, McpError> {
        match &self.kind {
            LocationKind::WholeFile => {
                let (source, parsed) = self.load()?;
                let content = parsed
                    .extract_symbols()
                    .map_err(|e| WorkspaceError::Parse {
                        path: self.relative_path.clone(),
                        error: e.to_string(),
                    })?
                    .to_string();
                Ok(ReadResult {
                    content,
                    source,
                    span: Span::WholeFile,
                    file_path: self.full_path.clone(),
                    relative_path: self.relative_path.clone(),
                })
            }
            LocationKind::Symbol(name) => self.read_symbol(name).map_err(Into::into),
            LocationKind::AnchorRange { .. } => {
                let source = self.read_source()?;
                let content = anchor::annotate(&source);
                Ok(ReadResult {
                    content,
                    source,
                    span: Span::WholeFile,
                    file_path: self.full_path.clone(),
                    relative_path: self.relative_path.clone(),
                })
            }
            LocationKind::SymbolWithAnchors { symbol, .. } => {
                self.read_symbol(symbol).map_err(Into::into)
            }
        }
    }

    pub fn save(&self, new_content: &str) -> Result<(), McpError> {
        if let LocationKind::WholeFile = &self.kind {
            return std::fs::write(&self.full_path, new_content)
                .map_err(|e| WorkspaceError::Io { path: self.relative_path.clone(), error: e }.into());
        }
        let result = self.read()?;
        match &self.kind {
            LocationKind::WholeFile => unreachable!(),
            LocationKind::Symbol(_) => result.overwrite(new_content).map_err(Into::into),
            LocationKind::AnchorRange { start, end }
            | LocationKind::SymbolWithAnchors { start, end, .. } => {
                let patched = anchor::patch(&result.content, start, end, new_content)
                    .map_err(WorkspaceError::from)?;
                result
                    .overwrite(&anchor::strip_annotations(&patched))
                    .map_err(Into::into)
            }
        }
    }

    fn read_symbol(&self, name: &str) -> Result<ReadResult, WorkspaceError> {
        let (source, parsed) = self.load()?;
        let (text, start_byte, end_byte) = parsed
            .find_symbol_with_range(name)
            .map_err(|e| WorkspaceError::Parse {
                path: self.relative_path.clone(),
                error: e.to_string(),
            })?
            .ok_or_else(|| WorkspaceError::SymbolNotFound {
                path: self.relative_path.clone(),
                symbol: name.to_string(),
            })?;
        Ok(ReadResult {
            content: anchor::annotate(&text),
            source,
            span: Span::Bytes(start_byte..end_byte),
            file_path: self.full_path.clone(),
            relative_path: self.relative_path.clone(),
        })
    }

    fn load(&self) -> Result<(String, ParsedSource), WorkspaceError> {
        let source = self.read_source()?;
        let language =
            LanguageType::try_from(self.full_path.as_path()).map_err(|_| WorkspaceError::UnsupportedFileType(self.relative_path.clone()))?;
        let parsed = ParsedSource::new(source.clone(), language).map_err(|e| WorkspaceError::Parse {
            path: self.relative_path.clone(),
            error: e.to_string(),
        })?;
        Ok((source, parsed))
    }

    fn read_source(&self) -> Result<String, WorkspaceError> {
        std::fs::read_to_string(&self.full_path).map_err(|e| WorkspaceError::Io {
            path: self.relative_path.clone(),
            error: e,
        })
    }
}

pub struct WorkspaceFile;

impl WorkspaceFile {
    pub fn locate(
        root: &Path,
        path: &str,
        symbol: Option<&str>,
        start_anchor: Option<&str>,
        end_anchor: Option<&str>,
    ) -> Result<ContentLocation, McpError> {
        let loc = FileLocation::new(root, path);

        let kind = match (symbol, start_anchor, end_anchor) {
            (None, None, None) => {
                Self::validate_write_path(&loc, root)?;
                LocationKind::WholeFile
            }
            (Some(s), None, None) => {
                Self::validate_path(&loc, root)?;
                LocationKind::Symbol(s.to_string())
            }
            (None, Some(start), Some(end)) => {
                Self::validate_path(&loc, root)?;
                LocationKind::AnchorRange {
                    start: start.to_string(),
                    end: end.to_string(),
                }
            }
            (Some(s), Some(start), Some(end)) => {
                Self::validate_path(&loc, root)?;
                LocationKind::SymbolWithAnchors {
                    symbol: s.to_string(),
                    start: start.to_string(),
                    end: end.to_string(),
                }
            }
            _ => return Err(WorkspaceError::InconsistentAnchors.into()),
        };

        Ok(ContentLocation {
            full_path: loc.full_path,
            relative_path: loc.relative_path,
            kind,
        })
    }

    fn validate_path(location: &FileLocation, workspace_root: &Path) -> Result<(), McpError> {
        Self::validate_write_path(location, workspace_root)?;

        if !location.as_ref().exists() {
            return Err(WorkspaceError::FileNotFound(location.relative_path.clone()).into());
        }

        if !location.as_ref().is_file() {
            return Err(WorkspaceError::NotAFile(location.relative_path.clone()).into());
        }

        Ok(())
    }

    fn validate_write_path(location: &FileLocation, workspace_root: &Path) -> Result<(), McpError> {
        if !location.as_ref().starts_with(workspace_root) {
            return Err(WorkspaceError::PathTraversal(location.relative_path.clone()).into());
        }
        Ok(())
    }
}
