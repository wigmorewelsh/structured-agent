use anyhow::Result;
use std::fmt;

#[derive(Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub parent_start_line: Option<usize>,
    pub trait_name: Option<String>,
}

impl Symbol {
    pub fn new(
        name: String,
        kind: String,
        start_line: usize,
        end_line: usize,
        start_byte: usize,
        end_byte: usize,
        trait_name: Option<String>,
    ) -> Self {
        Self {
            name,
            kind,
            start_line,
            end_line,
            start_byte,
            end_byte,
            parent_start_line: None,
            trait_name,
        }
    }

    fn is_child_of(&self, other: &Symbol) -> bool {
        self.start_line > other.start_line && self.end_line <= other.end_line
    }
}

pub struct SymbolOutline {
    symbols: Vec<Symbol>,
}

impl SymbolOutline {
    pub fn new(mut symbols: Vec<Symbol>) -> Self {
        Self::assign_parents(&mut symbols);
        symbols.sort_by_key(|s| s.start_line);
        Self { symbols }
    }

    pub fn find_byte_range(&self, name: &str) -> Option<(usize, usize)> {
        self.symbols
            .iter()
            .find(|s| s.name == name)
            .map(|s| (s.start_byte, s.end_byte))
    }

    pub fn find(&self, name: &str, source: &str) -> Result<Option<String>> {
        for symbol in &self.symbols {
            if symbol.name == name {
                return Self::extract_symbol_lines(symbol, source);
            }
        }
        Ok(None)
    }

    fn assign_parents(symbols: &mut [Symbol]) {
        for i in 0..symbols.len() {
            let closest_parent = Self::find_closest_parent(i, symbols);
            if let Some(parent_start) = closest_parent {
                symbols[i].parent_start_line = Some(parent_start);
            }
        }
    }

    fn find_closest_parent(child_index: usize, symbols: &[Symbol]) -> Option<usize> {
        let child = &symbols[child_index];
        let mut closest_parent_start: Option<usize> = None;

        for (j, parent) in symbols.iter().enumerate() {
            if child_index == j {
                continue;
            }

            if child.is_child_of(parent) {
                let is_closer = closest_parent_start
                    .map(|current| parent.start_line > current)
                    .unwrap_or(true);

                if is_closer {
                    closest_parent_start = Some(parent.start_line);
                }
            }
        }

        closest_parent_start
    }

    fn extract_symbol_lines(symbol: &Symbol, source: &str) -> Result<Option<String>> {
        let lines: Vec<&str> = source.lines().collect();
        if symbol.start_line > 0 && symbol.end_line <= lines.len() {
            let symbol_text = lines[(symbol.start_line - 1)..symbol.end_line].join("\n");
            Ok(Some(symbol_text))
        } else {
            Ok(None)
        }
    }

    fn format_symbol(&self, symbol: &Symbol) -> String {
        let display_name = self.get_display_name(symbol);
        let kind_display = self.format_kind(symbol, &display_name);
        format!(
            "{}-{} {}\n",
            symbol.start_line, symbol.end_line, kind_display
        )
    }

    fn get_display_name(&self, symbol: &Symbol) -> String {
        match symbol.parent_start_line {
            Some(parent_start) => {
                let parent_name = self
                    .symbols
                    .iter()
                    .find(|s| s.start_line == parent_start)
                    .map(|s| s.name.as_str())
                    .unwrap_or("");
                format!("{}::{}", parent_name, symbol.name)
            }
            None => symbol.name.clone(),
        }
    }

    fn format_kind(&self, symbol: &Symbol, display_name: &str) -> String {
        if symbol.kind == "impl" {
            Self::format_impl(symbol)
        } else {
            format!("{} {}", symbol.kind, display_name)
        }
    }

    fn format_impl(symbol: &Symbol) -> String {
        match &symbol.trait_name {
            Some(trait_name) => {
                format!("{} {} for {}", symbol.kind, trait_name, symbol.name)
            }
            None => format!("{} {}", symbol.kind, symbol.name),
        }
    }
}

impl fmt::Display for SymbolOutline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for symbol in &self.symbols {
            write!(f, "{}", self.format_symbol(symbol))?;
        }
        Ok(())
    }
}
