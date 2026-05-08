use crate::anchor::{self, PatchError};
use crate::parser::{LanguageType, ParsedSource, SymbolOutline};
use rmcp::ErrorData as McpError;
use serde_json::json;
use std::path::{Component, Path, PathBuf};

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

    fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

impl AsRef<Path> for FileLocation {
    fn as_ref(&self) -> &Path {
        &self.full_path
    }
}

pub struct WorkspaceFile {
    location: FileLocation,
    parsed: ParsedSource,
}

impl WorkspaceFile {
    pub fn open(workspace_root: &Path, relative_path: &str) -> Result<Self, McpError> {
        let location = FileLocation::new(workspace_root, relative_path);

        Self::validate_path(&location, workspace_root)?;

        let content = Self::read_content(&location)?;
        let language = Self::detect_language(&location)?;

        let parsed = ParsedSource::new(content, language).map_err(|e| {
            McpError::internal_error(
                format!("Failed to parse source: {}", e),
                Some(json!({"path": location.relative_path(), "language": format!("{:?}", language)})),
            )
        })?;

        Ok(Self { location, parsed })
    }

    pub fn relative_path(&self) -> &str {
        self.location.relative_path()
    }

    pub fn get_outline(&self) -> Result<String, McpError> {
        let outline = self.extract_symbols()?;
        Ok(outline.to_string())
    }

    pub fn replace_symbol(
        workspace_root: &Path,
        relative_path: &str,
        symbol_name: &str,
        replacement: &str,
    ) -> Result<(), McpError> {
        let file = Self::open(workspace_root, relative_path)?;
        let new_source = file
            .parsed
            .replace_symbol(symbol_name, replacement)
            .map_err(|e| {
                McpError::invalid_params(
                    format!("Failed to replace symbol: {}", e),
                    Some(json!({"path": relative_path, "symbol": symbol_name})),
                )
            })?;
        Self::write_content(&file.location, &new_source)
    }

    pub fn write_file(
        workspace_root: &Path,
        relative_path: &str,
        content: &str,
    ) -> Result<(), McpError> {
        let location = FileLocation::new(workspace_root, relative_path);
        Self::validate_write_path(&location, workspace_root)?;
        Self::write_content(&location, content)
    }

    pub fn get_symbol(&self, symbol_name: &str) -> Result<String, McpError> {
        let symbol_content = self.parsed.find_symbol(symbol_name).map_err(|e| {
            McpError::internal_error(
                format!("Failed to find symbol: {}", e),
                Some(json!({"path": self.relative_path(), "symbol": symbol_name})),
            )
        })?;

        let content = symbol_content.ok_or_else(|| {
            McpError::invalid_params(
                format!("Symbol '{}' not found", symbol_name),
                Some(json!({"path": self.relative_path(), "symbol": symbol_name})),
            )
        })?;

        Ok(anchor::annotate(&content))
    }

    pub fn patch_lines_with_anchors(
        workspace_root: &Path,
        relative_path: &str,
        start_anchor: &str,
        end_anchor: &str,
        replacement: &str,
    ) -> Result<(), McpError> {
        let file = Self::open(workspace_root, relative_path)?;
        let annotated = anchor::annotate(file.parsed.source());
        let patched = anchor::patch(&annotated, start_anchor, end_anchor, replacement).map_err(
            |e| match e {
                PatchError::AnchorNotFound(a) => McpError::invalid_params(
                    format!("Anchor not found: {}", a),
                    Some(json!({"path": relative_path, "anchor": a})),
                ),
                PatchError::AmbiguousAnchor(a) => McpError::invalid_params(
                    format!("Ambiguous anchor: {}", a),
                    Some(json!({"path": relative_path, "anchor": a})),
                ),
                PatchError::AnchorOrderInvalid => McpError::invalid_params(
                    "Start anchor comes after end anchor".to_string(),
                    Some(json!({"path": relative_path})),
                ),
            },
        )?;
        let new_content = anchor::strip_annotations(&patched);
        Self::write_content(&file.location, &new_content)
    }

    fn extract_symbols(&self) -> Result<SymbolOutline, McpError> {
        self.parsed.extract_symbols().map_err(|e| {
            McpError::internal_error(
                format!("Failed to extract symbols: {}", e),
                Some(json!({"path": self.relative_path()})),
            )
        })
    }

    fn validate_write_path(location: &FileLocation, workspace_root: &Path) -> Result<(), McpError> {
        if !location.as_ref().starts_with(workspace_root) {
            return Err(McpError::invalid_params(
                "Path escapes workspace root".to_string(),
                Some(json!({"path": location.relative_path()})),
            ));
        }
        Ok(())
    }

    fn write_content(location: &FileLocation, content: &str) -> Result<(), McpError> {
        std::fs::write(location.as_ref(), content).map_err(|e| {
            McpError::internal_error(
                format!("Failed to write file: {}", e),
                Some(json!({"path": location.relative_path()})),
            )
        })
    }

    fn validate_path(location: &FileLocation, workspace_root: &Path) -> Result<(), McpError> {
        if !location.as_ref().starts_with(workspace_root) {
            return Err(McpError::invalid_params(
                "Path escapes workspace root".to_string(),
                Some(json!({"path": location.relative_path()})),
            ));
        }

        if !location.as_ref().exists() {
            return Err(McpError::invalid_params(
                "File does not exist".to_string(),
                Some(json!({"path": location.relative_path()})),
            ));
        }

        if !location.as_ref().is_file() {
            return Err(McpError::invalid_params(
                "Path is not a file".to_string(),
                Some(json!({"path": location.relative_path()})),
            ));
        }

        Ok(())
    }

    fn read_content(location: &FileLocation) -> Result<String, McpError> {
        std::fs::read_to_string(location.as_ref()).map_err(|e| {
            McpError::internal_error(
                format!("Failed to read file: {}", e),
                Some(json!({"path": location.relative_path()})),
            )
        })
    }

    fn detect_language(location: &FileLocation) -> Result<LanguageType, McpError> {
        LanguageType::try_from(location.as_ref()).map_err(|_| {
            McpError::invalid_params(
                "Unsupported file type".to_string(),
                Some(json!({"path": location.relative_path()})),
            )
        })
    }
}
