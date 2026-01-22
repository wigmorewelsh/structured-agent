use tree_sitter::Language;

extern "C" {
    fn tree_sitter_structured_agent() -> Language;
}

pub fn language() -> Language {
    unsafe { tree_sitter_structured_agent() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_load_grammar() {
        let lang = language();
        assert_eq!(lang.node_kind_count() > 0, true);
    }
}
