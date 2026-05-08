mod common;

use insta::assert_snapshot;
use rmcp::handler::server::wrapper::Parameters;
use std::fs;
use structured_agent_workspace::parser::LanguageType;
use structured_agent_workspace::workspace::ReadFileRequest;

#[test]
fn test_language_type_from_path() {
    assert_eq!(
        LanguageType::try_from(std::path::Path::new("test.rs")).ok(),
        Some(LanguageType::Rust)
    );
    assert_eq!(
        LanguageType::try_from(std::path::Path::new("test.py")).ok(),
        Some(LanguageType::Python)
    );
    assert_eq!(
        LanguageType::try_from(std::path::Path::new("test.sh")).ok(),
        Some(LanguageType::Shell)
    );
    assert_eq!(
        LanguageType::try_from(std::path::Path::new("test.md")).ok(),
        Some(LanguageType::Markdown)
    );
    assert!(LanguageType::try_from(std::path::Path::new("test.txt")).is_err());
}

#[test]
fn test_read_rust_file_outline() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "test.rs".to_string(),
        symbol: None,
    }));
    assert!(result.is_ok());
    assert_snapshot!(
        result.unwrap().content[0]
            .as_text()
            .expect("Expected text content")
            .text
    );
}

#[test]
fn test_read_python_file_outline() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "test.py".to_string(),
        symbol: None,
    }));
    assert!(result.is_ok());
    assert_snapshot!(
        result.unwrap().content[0]
            .as_text()
            .expect("Expected text content")
            .text
    );
}

#[test]
fn test_read_shell_file_outline() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "test.sh".to_string(),
        symbol: None,
    }));
    assert!(result.is_ok());
    assert_snapshot!(
        result.unwrap().content[0]
            .as_text()
            .expect("Expected text content")
            .text
    );
}

#[test]
fn test_read_markdown_file_outline() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "test.md".to_string(),
        symbol: None,
    }));
    assert!(result.is_ok());
    assert_snapshot!(
        result.unwrap().content[0]
            .as_text()
            .expect("Expected text content")
            .text
    );
}

#[test]
fn test_read_specific_symbol() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "test.rs".to_string(),
        symbol: Some("Person".to_string()),
    }));
    assert!(result.is_ok());
    assert_snapshot!(
        result.unwrap().content[0]
            .as_text()
            .expect("Expected text content")
            .text
    );
}

#[test]
fn test_read_shell_specific_symbol() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "test.sh".to_string(),
        symbol: Some("greet".to_string()),
    }));
    assert!(result.is_ok());
    assert_snapshot!(
        result.unwrap().content[0]
            .as_text()
            .expect("Expected text content")
            .text
    );
}

#[test]
fn test_read_nonexistent_file() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "nonexistent.rs".to_string(),
        symbol: None,
    }));
    assert!(result.is_err());
}

#[test]
fn test_read_nonexistent_symbol() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "test.rs".to_string(),
        symbol: Some("NonexistentSymbol".to_string()),
    }));
    assert!(result.is_err());
}

#[test]
fn test_path_escapes_workspace() {
    let (_temp_dir, server) = common::create_test_workspace();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "../outside.rs".to_string(),
        symbol: None,
    }));
    assert!(result.is_err());
}

#[test]
fn test_unsupported_file_type() {
    let (temp_dir, server) = common::create_test_workspace();
    fs::write(temp_dir.path().join("test.txt"), "Some text").unwrap();
    let result = server.read_file(Parameters(ReadFileRequest {
        path: "test.txt".to_string(),
        symbol: None,
    }));
    assert!(result.is_err());
}
