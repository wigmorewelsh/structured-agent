mod common;

use rmcp::handler::server::wrapper::Parameters;
use structured_agent_workspace::workspace::{
    PatchLinesRequest, ReadFileRequest, ReplaceSymbolRequest, WriteFileRequest,
};

#[test]
fn test_patch_symbol_replaces_function() {
    let (temp_dir, server) = common::create_test_workspace();
    let new_body = "fn hello_world() {\n    println!(\"Goodbye!\");\n}";
    let result = server.replace_symbol(Parameters(ReplaceSymbolRequest {
        path: "test.rs".to_string(),
        symbol: "hello_world".to_string(),
        replacement: new_body.to_string(),
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert!(content.contains("Goodbye!"));
    assert!(!content.contains("Hello, world!"));
}

#[test]
fn test_patch_symbol_replaces_struct() {
    let (temp_dir, server) = common::create_test_workspace();
    let new_body = "struct Person {\n    name: String,\n}";
    let result = server.replace_symbol(Parameters(ReplaceSymbolRequest {
        path: "test.rs".to_string(),
        symbol: "Person".to_string(),
        replacement: new_body.to_string(),
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert!(content.contains("struct Person {"));
    assert!(!content.contains("struct Person {\n    name: String,\n    age: u32,"));
}

#[test]
fn test_patch_symbol_nonexistent_symbol() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.replace_symbol(Parameters(ReplaceSymbolRequest {
        path: "test.rs".to_string(),
        symbol: "nonexistent_symbol".to_string(),
        replacement: "fn nonexistent_symbol() {}".to_string(),
    }));
    assert!(result.is_err());
}

#[test]
fn test_patch_symbol_nonexistent_file() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.replace_symbol(Parameters(ReplaceSymbolRequest {
        path: "missing.rs".to_string(),
        symbol: "some_fn".to_string(),
        replacement: "fn some_fn() {}".to_string(),
    }));
    assert!(result.is_err());
}

#[test]
fn test_write_file_replaces_content() {
    let (temp_dir, server) = common::create_test_workspace();
    let new_content = "fn new_function() {}";
    let result = server.write_file(Parameters(WriteFileRequest {
        path: "test.rs".to_string(),
        content: new_content.to_string(),
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert_eq!(content, new_content);
}

#[test]
fn test_write_file_creates_new_file() {
    let (temp_dir, server) = common::create_test_workspace();
    let result = server.write_file(Parameters(WriteFileRequest {
        path: "new_file.rs".to_string(),
        content: "fn brand_new() {}".to_string(),
    }));
    assert!(result.is_ok());

    assert!(temp_dir.path().join("new_file.rs").exists());
}

#[test]
fn test_patch_lines_replaces_range() {
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

    let result = server.patch_lines(Parameters(PatchLinesRequest {
        path: "test.rs".to_string(),
        start_anchor: body_anchor.clone(),
        end_anchor: body_anchor,
        replacement: "    println!(\"Patched!\");".to_string(),
    }));
    assert!(result.is_ok());

    let content = std::fs::read_to_string(temp_dir.path().join("test.rs")).unwrap();
    assert!(content.contains("Patched!"));
    assert!(!content.contains("Hello, world!"));
}

#[test]
fn test_patch_lines_invalid_anchor() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.patch_lines(Parameters(PatchLinesRequest {
        path: "test.rs".to_string(),
        start_anchor: "deadbeef".to_string(),
        end_anchor: "deadbeef".to_string(),
        replacement: "x".to_string(),
    }));
    assert!(result.is_err());
}

#[test]
fn test_write_file_path_escapes_workspace() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.write_file(Parameters(WriteFileRequest {
        path: "../outside.rs".to_string(),
        content: "fn escape() {}".to_string(),
    }));
    assert!(result.is_err());
}
