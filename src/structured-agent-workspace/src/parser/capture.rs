use anyhow::{anyhow, Result};
use std::collections::HashSet;
use tree_sitter::Node;

use super::symbol::Symbol;

pub struct CaptureHandler<'a> {
    source_bytes: &'a [u8],
}

impl<'a> CaptureHandler<'a> {
    pub fn new(source_bytes: &'a [u8]) -> Self {
        Self { source_bytes }
    }

    pub fn process_match(
        &self,
        match_: &tree_sitter::QueryMatch,
        query: &tree_sitter::Query,
        collector: &mut SymbolCollector,
    ) -> Result<()> {
        let mut builder = SymbolBuilder::default();

        for capture in match_.captures {
            let capture_name = &query.capture_names()[capture.index as usize];
            self.handle_capture(capture_name, &capture.node, &mut builder)?;
        }

        if let Some(symbol) = builder.build()? {
            collector.add(symbol);
        }

        Ok(())
    }

    fn handle_capture<'b>(
        &self,
        capture_name: &str,
        node: &Node<'b>,
        builder: &mut SymbolBuilder<'b>,
    ) -> Result<()> {
        match CaptureType::from(capture_name) {
            CaptureType::Definition(kind) => builder.set_definition(kind, *node),
            CaptureType::Implementation => self.handle_implementation(node, builder)?,
            CaptureType::Name if builder.needs_name() => builder.set_name(self.node_text(node)?),
            _ => {}
        }
        Ok(())
    }

    fn handle_implementation<'b>(
        &self,
        node: &Node<'b>,
        builder: &mut SymbolBuilder<'b>,
    ) -> Result<()> {
        builder.set_kind("impl".to_string());
        builder.set_node(*node);

        let Some(type_name) = self.field_text(node, "type")? else {
            return Ok(());
        };

        builder.set_name(type_name.clone());

        if let Some(trait_name) = self.field_text(node, "trait")? {
            if trait_name != type_name {
                builder.set_trait(trait_name);
            }
        }

        Ok(())
    }

    fn field_text(&self, node: &Node, field: &str) -> Result<Option<String>> {
        Ok(node
            .child_by_field_name(field)
            .and_then(|n| n.utf8_text(self.source_bytes).ok())
            .map(String::from))
    }

    fn node_text(&self, node: &Node) -> Result<String> {
        node.utf8_text(self.source_bytes)
            .map(String::from)
            .map_err(|e| anyhow!("Failed to extract node text: {}", e))
    }
}

enum CaptureType {
    Definition(String),
    Implementation,
    Name,
    Other,
}

impl From<&str> for CaptureType {
    fn from(name: &str) -> Self {
        if let Some(kind) = name.strip_prefix("definition.") {
            Self::Definition(kind.to_string())
        } else if name.starts_with("reference.implementation") {
            Self::Implementation
        } else if name == "name" {
            Self::Name
        } else {
            Self::Other
        }
    }
}

#[derive(Default)]
struct SymbolBuilder<'a> {
    name: Option<String>,
    trait_name: Option<String>,
    node: Option<Node<'a>>,
    kind: String,
}

impl<'a> SymbolBuilder<'a> {
    fn set_definition(&mut self, kind: String, node: Node<'a>) {
        self.kind = kind;
        self.node = Some(node);
    }

    fn set_kind(&mut self, kind: String) {
        self.kind = kind;
    }

    fn set_node(&mut self, node: Node<'a>) {
        self.node = Some(node);
    }

    fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    fn set_trait(&mut self, trait_name: String) {
        self.trait_name = Some(trait_name);
    }

    fn needs_name(&self) -> bool {
        !self.kind.is_empty() && self.kind != "impl"
    }

    fn build(self) -> Result<Option<Symbol>> {
        match (self.name, self.node) {
            (Some(name), Some(node)) => {
                let start_pos = node.start_position();
                let end_pos = node.end_position();

                Ok(Some(Symbol::new(
                    name,
                    self.kind,
                    start_pos.row + 1,
                    end_pos.row + 1,
                    node.start_byte(),
                    node.end_byte(),
                    self.trait_name,
                )))
            }
            _ => Ok(None),
        }
    }
}

#[derive(Default)]
pub struct SymbolCollector {
    symbols: Vec<Symbol>,
    seen: HashSet<(usize, usize, String)>,
}

impl SymbolCollector {
    pub fn add(&mut self, symbol: Symbol) {
        let key = (symbol.start_line, symbol.end_line, symbol.name.clone());
        if self.seen.insert(key) {
            self.symbols.push(symbol);
        }
    }

    pub fn into_symbols(mut self) -> Vec<Symbol> {
        self.symbols.sort_by_key(|s| s.start_line);
        self.symbols
    }
}
