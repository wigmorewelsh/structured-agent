use rmcp::ErrorData as McpError;
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct WorkspaceDirectory {
    full_path: PathBuf,
    relative_path: String,
}

impl WorkspaceDirectory {
    pub fn open(workspace_root: &Path, relative_path: &str) -> Result<Self, McpError> {
        let full_path = workspace_root.join(relative_path);

        if !full_path.exists() {
            return Err(McpError::invalid_params(
                "Directory does not exist".to_string(),
                Some(json!({"path": relative_path})),
            ));
        }

        let canonical = full_path.canonicalize().map_err(|e| {
            McpError::internal_error(
                format!("Failed to resolve path: {}", e),
                Some(json!({"path": relative_path})),
            )
        })?;

        let canonical_root = workspace_root.canonicalize().map_err(|e| {
            McpError::internal_error(
                format!("Failed to resolve workspace root: {}", e),
                Some(json!({"path": relative_path})),
            )
        })?;

        if !canonical.starts_with(&canonical_root) {
            return Err(McpError::invalid_params(
                "Path escapes workspace root".to_string(),
                Some(json!({"path": relative_path})),
            ));
        }

        if !canonical.is_dir() {
            return Err(McpError::invalid_params(
                "Path is not a directory".to_string(),
                Some(json!({"path": relative_path})),
            ));
        }

        Ok(Self {
            full_path: canonical,
            relative_path: relative_path.to_string(),
        })
    }

    pub fn list(&self) -> String {
        let mut entries: Vec<String> = std::fs::read_dir(&self.full_path)
            .map(|rd| {
                rd.filter_map(|e| {
                    let entry = e.ok()?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if entry.path().is_dir() {
                        Some(format!("{}/", name))
                    } else {
                        Some(name)
                    }
                })
                .collect()
            })
            .unwrap_or_default();

        entries.sort();
        format!("{}:\n{}", self.relative_path, entries.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.rs"), "").unwrap();
        fs::write(tmp.path().join("b.txt"), "").unwrap();
        fs::create_dir(tmp.path().join("subdir")).unwrap();
        tmp
    }

    #[test]
    fn lists_files_and_dirs() {
        let tmp = setup();
        let dir = WorkspaceDirectory::open(tmp.path(), ".").unwrap();
        let output = dir.list();
        assert!(output.contains("a.rs"));
        assert!(output.contains("b.txt"));
        assert!(output.contains("subdir/"));
    }

    #[test]
    fn entries_are_sorted() {
        let tmp = setup();
        let dir = WorkspaceDirectory::open(tmp.path(), ".").unwrap();
        let output = dir.list();
        let lines: Vec<&str> = output.lines().skip(1).collect();
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted);
    }

    #[test]
    fn rejects_nonexistent_path() {
        let tmp = setup();
        let err = WorkspaceDirectory::open(tmp.path(), "missing").unwrap_err();
        assert!(err.message.contains("does not exist"));
    }

    #[test]
    fn rejects_file_path() {
        let tmp = setup();
        let err = WorkspaceDirectory::open(tmp.path(), "a.rs").unwrap_err();
        assert!(err.message.contains("not a directory"));
    }

    #[test]
    fn rejects_path_traversal() {
        let tmp = setup();
        let err = WorkspaceDirectory::open(tmp.path(), "../").unwrap_err();
        assert!(err.message.contains("escapes workspace root"));
    }
}
