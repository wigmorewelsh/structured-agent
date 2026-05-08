mod common;

use rmcp::handler::server::wrapper::Parameters;
use structured_agent_workspace::workspace::{EditFileRequest, ReadFileRequest};

#[test]
fn test_edit_file_replaces_symbol_function() {
    let (temp_dir, server) = common::create_test_workspace();
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "test.rs".to_string(),
        content: "fn hello_world() {\n    println!(\"Goodbye!\");\n}".to_string(),
        symbol: Some("hello_world".to_string()),
        start_anchor: None,
        end_anchor: None,
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert!(content.contains("Goodbye!"));
    assert!(!content.contains("Hello, world!"));
}

#[test]
fn test_edit_file_replaces_symbol_struct() {
    let (temp_dir, server) = common::create_test_workspace();
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "test.rs".to_string(),
        content: "struct Person {\n    name: String,\n}".to_string(),
        symbol: Some("Person".to_string()),
        start_anchor: None,
        end_anchor: None,
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert!(content.contains("struct Person {"));
    assert!(!content.contains("struct Person {\n    name: String,\n    age: u32,"));
}

#[test]
fn test_edit_file_nonexistent_symbol() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "test.rs".to_string(),
        content: "fn nonexistent() {}".to_string(),
        symbol: Some("nonexistent".to_string()),
        start_anchor: None,
        end_anchor: None,
    }));
    assert!(result.is_err());
}

#[test]
fn test_edit_file_nonexistent_file_with_symbol() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "missing.rs".to_string(),
        content: "fn some_fn() {}".to_string(),
        symbol: Some("some_fn".to_string()),
        start_anchor: None,
        end_anchor: None,
    }));
    assert!(result.is_err());
}

#[test]
fn test_edit_file_whole_file_write() {
    let (temp_dir, server) = common::create_test_workspace();
    let new_content = "fn new_function() {}";
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "test.rs".to_string(),
        content: new_content.to_string(),
        symbol: None,
        start_anchor: None,
        end_anchor: None,
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert_eq!(content, new_content);
}

#[test]
fn test_edit_file_creates_new_file() {
    let (temp_dir, server) = common::create_test_workspace();
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "new_file.rs".to_string(),
        content: "fn brand_new() {}".to_string(),
        symbol: None,
        start_anchor: None,
        end_anchor: None,
    }));
    assert!(result.is_ok());

    assert!(temp_dir.path().join("new_file.rs").exists());
}

#[test]
fn test_edit_file_path_escapes_workspace() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "../outside.rs".to_string(),
        content: "fn escape() {}".to_string(),
        symbol: None,
        start_anchor: None,
        end_anchor: None,
    }));
    assert!(result.is_err());
}

#[test]
fn test_edit_file_symbol_with_anchors() {
    let (temp_dir, server) = common::create_test_workspace();

    let read_result = server
        .read_file(Parameters(ReadFileRequest {
            path: "test.rs".to_string(),
            symbol: Some("hello_world".to_string()),
        }))
        .unwrap();
    let annotated = read_result.content[0].as_text().unwrap().text.clone();
    let lines: Vec<&str> = annotated.lines().collect();
    let body_anchor = lines[1].split_once('|').unwrap().0.to_string();

    let result = server.edit_file(Parameters(EditFileRequest {
        path: "test.rs".to_string(),
        content: "    println!(\"Patched within symbol!\");".to_string(),
        symbol: Some("hello_world".to_string()),
        start_anchor: Some(body_anchor.clone()),
        end_anchor: Some(body_anchor),
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert!(content.contains("Patched within symbol!"));
    assert!(!content.contains("Hello, world!"));
    assert!(content.contains("struct Person"));
}

#[test]
fn test_edit_file_anchor_patch() {
    let (temp_dir, server) = common::create_test_workspace();

    let read_result = server
        .read_file(Parameters(ReadFileRequest {
            path: "test.rs".to_string(),
            symbol: Some("hello_world".to_string()),
        }))
        .unwrap();
    let annotated = read_result.content[0].as_text().unwrap().text.clone();
    let lines: Vec<&str> = annotated.lines().collect();
    let body_anchor = lines[1].split_once('|').unwrap().0.to_string();

    let result = server.edit_file(Parameters(EditFileRequest {
        path: "test.rs".to_string(),
        content: "    println!(\"Patched!\");".to_string(),
        symbol: None,
        start_anchor: Some(body_anchor.clone()),
        end_anchor: Some(body_anchor),
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert!(content.contains("Patched!"));
    assert!(!content.contains("Hello, world!"));
}

#[test]
fn test_edit_file_invalid_anchor() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "test.rs".to_string(),
        content: "x".to_string(),
        symbol: None,
        start_anchor: Some("deadbeef".to_string()),
        end_anchor: Some("deadbeef".to_string()),
    }));
    assert!(result.is_err());
}

#[test]
fn test_edit_file_mismatched_anchors() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.edit_file(Parameters(EditFileRequest {
        path: "test.rs".to_string(),
        content: "x".to_string(),
        symbol: None,
        start_anchor: Some("deadbeef".to_string()),
        end_anchor: None,
    }));
    assert!(result.is_err());
}
