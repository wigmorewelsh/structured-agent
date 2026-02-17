use anyhow::{anyhow, Context, Result};
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

const RUST_OUTLINE_QUERY: &str = include_str!("../queries/rust-outline.scm");
const RUST_SYMBOL_QUERY: &str = include_str!("../queries/rust-symbol.scm");
const PYTHON_OUTLINE_QUERY: &str = include_str!("../queries/python-outline.scm");
const PYTHON_SYMBOL_QUERY: &str = include_str!("../queries/python-symbol.scm");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageType {
    Rust,
    Python,
}

impl LanguageType {
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext {
                "rs" => Some(Self::Rust),
                "py" => Some(Self::Python),
                _ => None,
            })
    }

    pub fn language(&self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
        }
    }

    fn outline_query(&self) -> &'static str {
        match self {
            Self::Rust => RUST_OUTLINE_QUERY,
            Self::Python => PYTHON_OUTLINE_QUERY,
        }
    }

    fn symbol_query(&self, symbol_name: &str) -> String {
        let template = match self {
            Self::Rust => RUST_SYMBOL_QUERY,
            Self::Python => PYTHON_SYMBOL_QUERY,
        };
        template.replace("{symbol}", symbol_name)
    }
}

#[derive(Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    #[allow(dead_code)]
    pub start_byte: usize,
    #[allow(dead_code)]
    pub end_byte: usize,
    pub parent_start_line: Option<usize>,
    pub trait_name: Option<String>,
}

pub struct FileParser {
    parser: Parser,
    language_type: LanguageType,
}

impl FileParser {
    pub fn new(language_type: LanguageType) -> Result<Self> {
        let mut parser = Parser::new();
        let language = language_type.language();
        parser
            .set_language(&language)
            .context("Failed to set parser language")?;

        Ok(Self {
            parser,
            language_type,
        })
    }

    pub fn parse(&mut self, source: &str) -> Result<Tree> {
        self.parser
            .parse(source, None)
            .ok_or_else(|| anyhow!("Failed to parse source code"))
    }

    pub fn get_outline(&mut self, source: &str) -> Result<Vec<Symbol>> {
        let tree = self.parse(source)?;
        let query_str = self.language_type.outline_query();
        let query = Query::new(&self.language_type.language(), query_str)
            .context("Failed to create outline query")?;

        let mut cursor = QueryCursor::new();
        let source_bytes = source.as_bytes();

        let mut symbols = Vec::new();

        let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);

        while let Some(match_) = matches.next() {
            let mut name: Option<String> = None;
            let mut trait_name: Option<String> = None;
            let mut def_node = None;
            let mut kind = String::new();

            for capture in match_.captures {
                let capture_name = &query.capture_names()[capture.index as usize];

                if capture_name.ends_with(".name") || capture_name.ends_with(".type") {
                    name = Some(capture.node.utf8_text(source_bytes)?.to_string());
                    kind = capture_name
                        .strip_suffix(".name")
                        .or_else(|| capture_name.strip_suffix(".type"))
                        .unwrap_or("")
                        .to_string();
                } else if capture_name.ends_with(".trait") {
                    trait_name = Some(capture.node.utf8_text(source_bytes)?.to_string());
                } else if capture_name.ends_with(".def") {
                    def_node = Some(capture.node);
                }
            }

            if let (Some(name_str), Some(node)) = (name, def_node) {
                let start_pos = node.start_position();
                let end_pos = node.end_position();

                symbols.push(Symbol {
                    name: name_str,
                    kind,
                    start_line: start_pos.row + 1,
                    end_line: end_pos.row + 1,
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    parent_start_line: None,
                    trait_name,
                });
            }
        }

        symbols.sort_by_key(|s| s.start_line);

        for i in 0..symbols.len() {
            for j in 0..symbols.len() {
                if i != j {
                    let (child_start, child_end) = (symbols[i].start_line, symbols[i].end_line);
                    let (parent_start, parent_end) = (symbols[j].start_line, symbols[j].end_line);

                    if child_start > parent_start && child_end <= parent_end {
                        if symbols[i].parent_start_line.is_none()
                            || symbols[i].parent_start_line.unwrap() < parent_start
                        {
                            symbols[i].parent_start_line = Some(parent_start);
                        }
                    }
                }
            }
        }

        Ok(symbols)
    }

    pub fn get_symbol(&mut self, source: &str, symbol_name: &str) -> Result<Option<String>> {
        let tree = self.parse(source)?;
        let query_str = self.language_type.symbol_query(symbol_name);
        let query = Query::new(&self.language_type.language(), &query_str)
            .context("Failed to create symbol query")?;

        let mut cursor = QueryCursor::new();
        let source_bytes = source.as_bytes();

        let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &query.capture_names()[capture.index as usize];
                if *capture_name == "definition" {
                    let text = capture.node.utf8_text(source_bytes)?;
                    return Ok(Some(text.to_string()));
                }
            }
        }

        Ok(None)
    }
}
