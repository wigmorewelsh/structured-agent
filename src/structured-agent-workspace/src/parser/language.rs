use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tree_sitter::{Language, Query};

const BASH_TAGS_QUERY: &str = r#"
(function_definition
  name: (word) @name) @definition.function
"#;

const MARKDOWN_TAGS_QUERY: &str = r#"
(atx_heading
  heading_content: (_) @name) @definition.section
(setext_heading
  heading_content: (_) @name) @definition.section
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageType {
    Rust,
    Python,
    Shell,
    Markdown,
}

impl TryFrom<&Path> for LanguageType {
    type Error = anyhow::Error;

    fn try_from(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| anyhow!("File has no extension"))?;

        match ext {
            "rs" => Ok(Self::Rust),
            "py" => Ok(Self::Python),
            "sh" => Ok(Self::Shell),
            "md" => Ok(Self::Markdown),
            _ => Err(anyhow!("Unsupported file extension: {}", ext)),
        }
    }
}

impl LanguageType {
    pub fn language(&self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Shell => tree_sitter_bash::LANGUAGE.into(),
            Self::Markdown => tree_sitter_md::LANGUAGE.into(),
        }
    }

    pub fn tags_query(&self) -> &'static str {
        match self {
            Self::Rust => tree_sitter_rust::TAGS_QUERY,
            Self::Python => tree_sitter_python::TAGS_QUERY,
            Self::Shell => BASH_TAGS_QUERY,
            Self::Markdown => MARKDOWN_TAGS_QUERY,
        }
    }

    pub fn create_query(&self) -> Result<Query> {
        let query_str = self.tags_query();
        Query::new(&self.language(), query_str).context("Failed to create tags query")
    }
}
