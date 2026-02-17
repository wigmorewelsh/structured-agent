use anyhow::{anyhow, Context, Result};
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

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

    fn tags_query(&self) -> &'static str {
        match self {
            Self::Rust => tree_sitter_rust::TAGS_QUERY,
            Self::Python => tree_sitter_python::TAGS_QUERY,
        }
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
        let query_str = self.language_type.tags_query();
        let query = Query::new(&self.language_type.language(), query_str)
            .context("Failed to create tags query")?;

        let mut cursor = QueryCursor::new();
        let source_bytes = source.as_bytes();

        let mut symbols = Vec::new();
        let mut seen_definitions = std::collections::HashSet::new();

        let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);

        while let Some(match_) = matches.next() {
            let mut name: Option<String> = None;
            let mut trait_name: Option<String> = None;
            let mut def_node = None;
            let mut kind = String::new();

            for capture in match_.captures {
                let capture_name = &query.capture_names()[capture.index as usize];

                if capture_name.starts_with("definition.") {
                    kind = capture_name
                        .strip_prefix("definition.")
                        .unwrap_or("")
                        .to_string();
                    def_node = Some(capture.node);
                } else if capture_name.starts_with("reference.implementation") {
                    kind = "impl".to_string();
                    def_node = Some(capture.node);
                    if let Some((impl_type, impl_trait)) =
                        self.extract_impl_info(&capture.node, source_bytes)?
                    {
                        name = Some(impl_type.clone());
                        if impl_trait != impl_type {
                            trait_name = Some(impl_trait);
                        }
                    }
                } else if *capture_name == "name" && !kind.is_empty() && kind != "impl" {
                    name = Some(capture.node.utf8_text(source_bytes)?.to_string());
                }
            }

            if let (Some(name_str), Some(node)) = (name, def_node) {
                let start_pos = node.start_position();
                let end_pos = node.end_position();
                let key = (start_pos.row, end_pos.row, name_str.clone());

                if seen_definitions.insert(key) {
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

    fn extract_impl_info(
        &self,
        impl_node: &tree_sitter::Node,
        source_bytes: &[u8],
    ) -> Result<Option<(String, String)>> {
        let impl_type = if let Some(type_node) = impl_node.child_by_field_name("type") {
            type_node.utf8_text(source_bytes).ok().map(String::from)
        } else {
            None
        };

        let impl_trait = if let Some(trait_node) = impl_node.child_by_field_name("trait") {
            trait_node.utf8_text(source_bytes).ok().map(String::from)
        } else {
            None
        };

        if let Some(typ) = impl_type {
            if let Some(trt) = impl_trait {
                Ok(Some((typ, trt)))
            } else {
                Ok(Some((typ.clone(), typ)))
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_symbol(&mut self, source: &str, symbol_name: &str) -> Result<Option<String>> {
        let symbols = self.get_outline(source)?;

        for symbol in symbols {
            if symbol.name == symbol_name {
                let lines: Vec<&str> = source.lines().collect();
                if symbol.start_line > 0 && symbol.end_line <= lines.len() {
                    let symbol_text = lines[(symbol.start_line - 1)..symbol.end_line].join("\n");
                    return Ok(Some(symbol_text));
                }
            }
        }

        Ok(None)
    }
}
