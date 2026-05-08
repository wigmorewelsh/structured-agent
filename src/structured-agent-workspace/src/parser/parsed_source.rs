use anyhow::{anyhow, Result};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor, Tree};

use super::language::LanguageType;
use super::symbol::SymbolOutline;

use super::capture::{CaptureHandler, SymbolCollector};

pub struct ParsedSource {
    source: String,
    language: LanguageType,
    tree: Tree,
}

impl ParsedSource {
    pub fn new(source: String, language: LanguageType) -> Result<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&language.language())
            .map_err(|e| anyhow!("Failed to set parser language: {}", e))?;

        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| anyhow!("Failed to parse source code"))?;

        Ok(Self {
            source,
            language,
            tree,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    pub fn language(&self) -> LanguageType {
        self.language
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn extract_symbols(&self) -> Result<SymbolOutline> {
        let query = self.language.create_query()?;
        let symbols = self.query_symbols(&query)?;
        Ok(SymbolOutline::new(symbols))
    }

    pub fn replace_symbol(&self, symbol_name: &str, replacement: &str) -> Result<String> {
        let outline = self.extract_symbols()?;
        let (start_byte, end_byte) = outline
            .find_byte_range(symbol_name)
            .ok_or_else(|| anyhow!("Symbol '{}' not found", symbol_name))?;
        let mut new_source = self.source.clone();
        new_source.replace_range(start_byte..end_byte, replacement);
        Ok(new_source)
    }

    pub fn find_symbol(&self, symbol_name: &str) -> Result<Option<String>> {
        let outline = self.extract_symbols()?;
        outline.find(symbol_name, &self.source)
    }

    fn query_symbols(&self, query: &Query) -> Result<Vec<super::symbol::Symbol>> {
        let handler = CaptureHandler::new(self.source_bytes());
        let mut collector = SymbolCollector::default();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), self.source_bytes());

        while let Some(match_) = matches.next() {
            handler.process_match(match_, query, &mut collector)?;
        }

        Ok(collector.into_symbols())
    }
}
