const HASH_LEN: usize = 8;

pub fn hash_line(line: &str) -> String {
    let hash = blake3::hash(line.as_bytes());
    hash.to_hex()[..HASH_LEN].to_string()
}

pub fn annotate(content: &str) -> String {
    content
        .lines()
        .map(|line| format!("{}|{}\n", hash_line(line), line))
        .collect()
}

#[derive(Debug, PartialEq)]
pub enum PatchError {
    AnchorNotFound(String),
    AmbiguousAnchor(String),
    AnchorOrderInvalid,
}

pub fn patch(
    annotated: &str,
    start_anchor: &str,
    end_anchor: &str,
    replacement: &str,
) -> Result<String, PatchError> {
    let parsed: Vec<(&str, &str)> = annotated
        .lines()
        .filter_map(|line| line.split_once('|'))
        .collect();

    let start_matches: Vec<usize> = parsed
        .iter()
        .enumerate()
        .filter(|(_, (h, _))| *h == start_anchor)
        .map(|(i, _)| i)
        .collect();

    let end_matches: Vec<usize> = parsed
        .iter()
        .enumerate()
        .filter(|(_, (h, _))| *h == end_anchor)
        .map(|(i, _)| i)
        .collect();

    if start_matches.is_empty() {
        return Err(PatchError::AnchorNotFound(start_anchor.to_string()));
    }
    if end_matches.is_empty() {
        return Err(PatchError::AnchorNotFound(end_anchor.to_string()));
    }
    if start_matches.len() > 1 {
        return Err(PatchError::AmbiguousAnchor(start_anchor.to_string()));
    }
    if end_matches.len() > 1 {
        return Err(PatchError::AmbiguousAnchor(end_anchor.to_string()));
    }

    let start_idx = start_matches[0];
    let end_idx = end_matches[0];

    if start_idx > end_idx {
        return Err(PatchError::AnchorOrderInvalid);
    }

    let mut result = String::new();
    for (i, (hash, content)) in parsed.iter().enumerate() {
        if i < start_idx || i > end_idx {
            result.push_str(&format!("{}|{}\n", hash, content));
        } else if i == start_idx {
            for new_line in replacement.lines() {
                result.push_str(&format!("{}|{}\n", hash_line(new_line), new_line));
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotate_empty_string() {
        assert_eq!(annotate(""), "");
    }

    #[test]
    fn annotate_prefixes_each_line_with_hash() {
        let result = annotate("foo\nbar");
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("|foo"));
        assert!(lines[1].contains("|bar"));
    }

    #[test]
    fn annotate_hash_is_consistent() {
        let a = annotate("hello world");
        let b = annotate("hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn annotate_different_lines_produce_different_hashes() {
        let result = annotate("foo\nbar");
        let lines: Vec<&str> = result.lines().collect();
        let hash_foo = lines[0].split_once('|').unwrap().0;
        let hash_bar = lines[1].split_once('|').unwrap().0;
        assert_ne!(hash_foo, hash_bar);
    }

    #[test]
    fn annotate_hash_length_is_correct() {
        let result = annotate("some line");
        let hash = result.lines().next().unwrap().split_once('|').unwrap().0;
        assert_eq!(hash.len(), HASH_LEN);
    }

    #[test]
    fn patch_replaces_single_line() {
        let annotated = annotate("line one\nline two\nline three");
        let lines: Vec<&str> = annotated.lines().collect();
        let anchor = lines[1].split_once('|').unwrap().0;

        let result = patch(&annotated, anchor, anchor, "replaced").unwrap();
        let result_lines: Vec<&str> = result.lines().collect();

        assert_eq!(result_lines.len(), 3);
        assert!(result_lines[1].ends_with("|replaced"));
    }

    #[test]
    fn patch_replaces_line_range() {
        let annotated = annotate("a\nb\nc\nd");
        let lines: Vec<&str> = annotated.lines().collect();
        let start = lines[1].split_once('|').unwrap().0;
        let end = lines[2].split_once('|').unwrap().0;

        let result = patch(&annotated, start, end, "x\ny\nz").unwrap();
        let result_lines: Vec<&str> = result.lines().collect();

        assert_eq!(result_lines.len(), 5);
        assert!(result_lines[0].ends_with("|a"));
        assert!(result_lines[1].ends_with("|x"));
        assert!(result_lines[2].ends_with("|y"));
        assert!(result_lines[3].ends_with("|z"));
        assert!(result_lines[4].ends_with("|d"));
    }

    #[test]
    fn patch_error_when_start_anchor_not_found() {
        let annotated = annotate("foo\nbar");
        let err = patch(&annotated, "deadbeef", "deadbeef", "x").unwrap_err();
        assert_eq!(err, PatchError::AnchorNotFound("deadbeef".to_string()));
    }

    #[test]
    fn patch_error_when_end_anchor_not_found() {
        let annotated = annotate("foo\nbar");
        let lines: Vec<&str> = annotated.lines().collect();
        let start = lines[0].split_once('|').unwrap().0;

        let err = patch(&annotated, start, "deadbeef", "x").unwrap_err();
        assert_eq!(err, PatchError::AnchorNotFound("deadbeef".to_string()));
    }

    #[test]
    fn patch_error_on_ambiguous_start_anchor() {
        let annotated = annotate("same\nsame\ndifferent");
        let lines: Vec<&str> = annotated.lines().collect();
        let anchor = lines[0].split_once('|').unwrap().0;

        let err = patch(&annotated, anchor, anchor, "x").unwrap_err();
        assert_eq!(err, PatchError::AmbiguousAnchor(anchor.to_string()));
    }

    #[test]
    fn patch_error_on_anchor_order_invalid() {
        let annotated = annotate("a\nb\nc");
        let lines: Vec<&str> = annotated.lines().collect();
        let anchor_c = lines[2].split_once('|').unwrap().0;
        let anchor_a = lines[0].split_once('|').unwrap().0;

        let err = patch(&annotated, anchor_c, anchor_a, "x").unwrap_err();
        assert_eq!(err, PatchError::AnchorOrderInvalid);
    }

    #[test]
    fn patch_unchanged_lines_retain_their_hashes() {
        let annotated = annotate("a\nb\nc");
        let lines: Vec<&str> = annotated.lines().collect();
        let anchor_b = lines[1].split_once('|').unwrap().0;
        let hash_a_before = lines[0].split_once('|').unwrap().0.to_string();
        let hash_c_before = lines[2].split_once('|').unwrap().0.to_string();

        let result = patch(&annotated, anchor_b, anchor_b, "replaced").unwrap();
        let result_lines: Vec<&str> = result.lines().collect();

        let hash_a_after = result_lines[0].split_once('|').unwrap().0;
        let hash_c_after = result_lines[2].split_once('|').unwrap().0;

        assert_eq!(hash_a_before, hash_a_after);
        assert_eq!(hash_c_before, hash_c_after);
    }

    #[test]
    fn patch_new_lines_have_content_derived_hashes() {
        let annotated = annotate("a\nb\nc");
        let lines: Vec<&str> = annotated.lines().collect();
        let anchor_b = lines[1].split_once('|').unwrap().0;

        let result = patch(&annotated, anchor_b, anchor_b, "new line").unwrap();
        let result_lines: Vec<&str> = result.lines().collect();
        let new_hash = result_lines[1].split_once('|').unwrap().0;

        assert_eq!(new_hash, hash_line("new line"));
    }
}
