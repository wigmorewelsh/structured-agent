use insta::assert_snapshot;
use rmcp::handler::server::wrapper::Parameters;
use std::fs;
use structured_agent_workspace::parser::LanguageType;
use structured_agent_workspace::workspace::{ReadFileRequest, WorkspaceServer};
use tempfile::TempDir;

const TEST_RS_FIXTURE: &str = include_str!("fixtures/test.rs");
const TEST_PY_FIXTURE: &str = include_str!("fixtures/test.py");

fn create_test_workspace() -> (TempDir, WorkspaceServer) {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();

    fs::write(workspace_root.join("test.rs"), TEST_RS_FIXTURE).unwrap();
    fs::write(workspace_root.join("test.py"), TEST_PY_FIXTURE).unwrap();

    let server = WorkspaceServer::new(workspace_root);
    (temp_dir, server)
}

#[test]
fn test_language_type_from_path() {
    let rust_path = std::path::Path::new("test.rs");
    let python_path = std::path::Path::new("test.py");
    let unknown_path = std::path::Path::new("test.txt");

    assert_eq!(LanguageType::from_path(rust_path), Some(LanguageType::Rust));
    assert_eq!(
        LanguageType::from_path(python_path),
        Some(LanguageType::Python)
    );
    assert_eq!(LanguageType::from_path(unknown_path), None);
}

#[test]
fn test_read_rust_file_outline() {
    let (_temp_dir, server) = create_test_workspace();

    let request = ReadFileRequest {
        path: "test.rs".to_string(),
        symbol: None,
    };

    let result = server.read_file(Parameters(request));
    assert!(result.is_ok());

    let tool_result = result.unwrap();
    let content = &tool_result.content[0];

    let text_str = content.as_text().expect("Expected text content");

    assert_snapshot!(text_str.text);
}

#[test]
fn test_read_python_file_outline() {
    let (_temp_dir, server) = create_test_workspace();

    let request = ReadFileRequest {
        path: "test.py".to_string(),
        symbol: None,
    };

    let result = server.read_file(Parameters(request));
    assert!(result.is_ok());

    let tool_result = result.unwrap();
    let content = &tool_result.content[0];

    let text_str = content.as_text().expect("Expected text content");

    assert_snapshot!(text_str.text);
}

#[test]
fn test_read_specific_symbol() {
    let (_temp_dir, server) = create_test_workspace();

    let request = ReadFileRequest {
        path: "test.rs".to_string(),
        symbol: Some("Person".to_string()),
    };

    let result = server.read_file(Parameters(request));
    assert!(result.is_ok());

    let tool_result = result.unwrap();
    let content = &tool_result.content[0];

    let text_str = content.as_text().expect("Expected text content");

    assert_snapshot!(text_str.text);
}

#[test]
fn test_read_nonexistent_file() {
    let (_temp_dir, server) = create_test_workspace();

    let request = ReadFileRequest {
        path: "nonexistent.rs".to_string(),
        symbol: None,
    };

    let result = server.read_file(Parameters(request));
    assert!(result.is_err());
}

#[test]
fn test_read_nonexistent_symbol() {
    let (_temp_dir, server) = create_test_workspace();

    let request = ReadFileRequest {
        path: "test.rs".to_string(),
        symbol: Some("NonexistentSymbol".to_string()),
    };

    let result = server.read_file(Parameters(request));
    assert!(result.is_err());
}

#[test]
fn test_path_escapes_workspace() {
    let (_temp_dir, server) = create_test_workspace();

    let request = ReadFileRequest {
        path: "../outside.rs".to_string(),
        symbol: None,
    };

    let result = server.read_file(Parameters(request));
    assert!(result.is_err());
}

#[test]
fn test_unsupported_file_type() {
    let (temp_dir, server) = create_test_workspace();

    fs::write(temp_dir.path().join("test.txt"), "Some text").unwrap();

    let request = ReadFileRequest {
        path: "test.txt".to_string(),
        symbol: None,
    };

    let result = server.read_file(Parameters(request));
    assert!(result.is_err());
}
