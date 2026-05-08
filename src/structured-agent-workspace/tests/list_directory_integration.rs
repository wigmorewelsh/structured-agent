mod common;

use rmcp::handler::server::wrapper::Parameters;
use std::fs;
use structured_agent_workspace::workspace::ListDirectoryRequest;

#[test]
fn test_list_directory_root() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.list_directory(Parameters(ListDirectoryRequest {
        path: ".".to_string(),
    }));
    assert!(result.is_ok());
    let text = result.unwrap().content[0]
        .as_text()
        .expect("Expected text content")
        .text
        .clone();
    assert!(text.contains("test.rs"));
    assert!(text.contains("test.py"));
}

#[test]
fn test_list_directory_subdirectory() {
    let (temp_dir, server) = common::create_test_workspace();
    fs::create_dir(temp_dir.path().join("subdir")).unwrap();
    fs::write(temp_dir.path().join("subdir/nested.rs"), "").unwrap();
    let result = server.list_directory(Parameters(ListDirectoryRequest {
        path: "subdir".to_string(),
    }));
    assert!(result.is_ok());
    let text = result.unwrap().content[0]
        .as_text()
        .expect("Expected text content")
        .text
        .clone();
    assert!(text.contains("nested.rs"));
}

#[test]
fn test_list_directory_nonexistent() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.list_directory(Parameters(ListDirectoryRequest {
        path: "missing".to_string(),
    }));
    assert!(result.is_err());
}

#[test]
fn test_list_directory_path_traversal() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.list_directory(Parameters(ListDirectoryRequest {
        path: "../".to_string(),
    }));
    assert!(result.is_err());
}

#[test]
fn test_list_directory_on_file() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.list_directory(Parameters(ListDirectoryRequest {
        path: "test.rs".to_string(),
    }));
    assert!(result.is_err());
}
