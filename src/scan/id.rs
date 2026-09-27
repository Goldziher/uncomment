//! Stable identifiers for comments, linking a `scan` inventory to a later `keep` run.
//!
//! ```text
//! id = blake3(repo_relative_path_with_slashes || 0x00 || exact_comment_bytes || 0x00
//!             || occurrence_index_le_bytes)
//!      truncated to 5 bytes, rendered as 10 lowercase hex chars
//! ```
//!
//! Line and byte offsets are deliberately *not* part of the input: the file is expected to change
//! between the two runs, and an id that moved every time a line was inserted above a comment would
//! be useless. What identifies a comment is its path, its exact bytes, and which of the
//! byte-identical comments in that file it is.
//!
//! Everything here is a pure function of its arguments — no hash-map iteration order, no
//! `DefaultHasher`, no addresses, no non-ASCII case folding beyond what `str::to_lowercase`
//! specifies — so two runs on two machines produce the same ids for the same bytes.
//!
//! Five bytes is 40 bits, where a birthday collision becomes plausible around a million comments,
//! so the truncation is treated as fallible: [`ambiguous_ids`] reports which ids more than one
//! comment produced, and [`full_id`] widens just those.

use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Bytes of digest in a short id — 10 hex characters.
pub const ID_BYTES: usize = 5;

/// Bytes of digest in a [`full_id`] — 32 hex characters.
pub const FULL_ID_BYTES: usize = 16;

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Identifier for one comment, as emitted by `scan` and read back by `keep`.
pub fn comment_id(repo_relative_path: &str, comment_text: &str, occurrence_index: u32) -> String {
    truncate_hex(
        &comment_digest(repo_relative_path, comment_text, occurrence_index),
        ID_BYTES,
    )
}

/// The same identifier at full width, for the rare pair that collides at [`ID_BYTES`].
pub fn full_id(repo_relative_path: &str, comment_text: &str, occurrence_index: u32) -> String {
    truncate_hex(
        &comment_digest(repo_relative_path, comment_text, occurrence_index),
        FULL_ID_BYTES,
    )
}

/// Decision key for a comment's normalized text, independent of location.
///
/// No path and no occurrence index, so "this wording is always worth keeping" is a decision that
/// carries across files, and across repositories.
pub fn group_id(normalized_text: &str) -> String {
    truncate_hex(blake3::hash(normalized_text.as_bytes()).as_bytes(), ID_BYTES)
}

/// Render the first `bytes` of `digest` as lowercase hex.
///
/// Exposed so a caller can widen an id it found to be ambiguous, and so the widening path is
/// testable against hand-built digests instead of a search for a real blake3 collision. `bytes` is
/// clamped to the digest width.
pub fn truncate_hex(digest: &[u8; 32], bytes: usize) -> String {
    let bytes = bytes.clamp(1, digest.len());
    let mut out = String::with_capacity(bytes * 2);
    for byte in &digest[..bytes] {
        out.push(HEX[usize::from(byte >> 4)] as char);
        out.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    out
}

/// Ids that more than one comment produced, in the order they first appear in `ids`.
///
/// `scan` widens these with [`full_id`] and notes them on stderr; `keep` must refuse to act on one,
/// because marking the wrong comment is worse than reporting that it cannot tell them apart.
pub fn ambiguous_ids<S: AsRef<str>>(ids: &[S]) -> Vec<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for id in ids {
        *counts.entry(id.as_ref()).or_insert(0) += 1;
    }

    let mut reported: BTreeSet<&str> = BTreeSet::new();
    let mut out = Vec::new();
    for id in ids {
        let id = id.as_ref();
        if counts.get(id).copied().unwrap_or(0) > 1 && reported.insert(id) {
            out.push(id.to_string());
        }
    }
    out
}

/// Assign occurrence indices to a file's comments: 0-based among comments in that file with
/// byte-identical text.
pub fn assign_occurrence_indices(texts: &[&str]) -> Vec<u32> {
    let mut seen: HashMap<&str, u32> = HashMap::with_capacity(texts.len());
    texts
        .iter()
        .map(|text| {
            let next = seen.entry(text).or_insert(0);
            let index = *next;
            *next = next.saturating_add(1);
            index
        })
        .collect()
}

/// Delimiters stripped, whitespace collapsed, lowercased.
///
/// The point is that a block comment and the equivalent run of line comments normalize to the same
/// string, so a decision taken about one applies to the other.
pub fn normalize_comment_text(text: &str) -> String {
    let mut joined = String::with_capacity(text.len());
    for line in text.lines() {
        joined.push(' ');
        joined.push_str(strip_delimiters(line));
    }

    let mut out = String::with_capacity(joined.len());
    for word in joined.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out.to_lowercase()
}

const LINE_PREFIXES: &[&str] = &["///", "//!", "//", "/**", "/*!", "/*", "\"\"\"", "'''", "--", "#"];
const LINE_SUFFIXES: &[&str] = &["*/", "\"\"\"", "'''"];

fn strip_delimiters(line: &str) -> &str {
    let mut line = line.trim();

    for suffix in LINE_SUFFIXES {
        if let Some(rest) = line.strip_suffix(suffix) {
            line = rest.trim_end();
            break;
        }
    }

    for prefix in LINE_PREFIXES {
        if let Some(rest) = line.strip_prefix(prefix) {
            line = rest.trim_start();
            break;
        }
    }

    // A block comment's continuation lines start with `*`. Handled after the suffix strip above so
    // that a closing `*/` is already gone and cannot be mistaken for one.
    if let Some(rest) = line.strip_prefix('*')
        && !rest.starts_with('/')
    {
        line = rest.trim_start();
    }

    line.trim()
}

fn comment_digest(repo_relative_path: &str, comment_text: &str, occurrence_index: u32) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    // The 0x00 separators keep the fields unambiguous: without them a path ending in `x` followed by
    // a comment starting with `y` would hash the same as the path `xy` and a comment missing its
    // first byte.
    hasher.update(repo_relative_path.as_bytes());
    hasher.update(&[0]);
    hasher.update(comment_text.as_bytes());
    hasher.update(&[0]);
    hasher.update(&occurrence_index.to_le_bytes());
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids for every comment in one file, mirroring what `scan` does per file.
    fn ids_for(path: &str, texts: &[&str]) -> Vec<String> {
        let indices = assign_occurrence_indices(texts);
        texts
            .iter()
            .zip(indices)
            .map(|(text, index)| comment_id(path, text, index))
            .collect()
    }

    fn line_comments(content: &str) -> Vec<&str> {
        content
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("//"))
            .collect()
    }

    #[test]
    fn an_id_is_ten_lowercase_hex_characters() {
        let id = comment_id("src/lib.rs", "// keep me", 0);
        assert_eq!(id.len(), ID_BYTES * 2);
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{id}"
        );
    }

    #[test]
    fn ids_are_pinned_so_a_future_change_to_the_scheme_is_visible() {
        // Golden values: `keep` reads ids written by an earlier `scan`, possibly by another build on
        // another machine, so the scheme cannot drift silently.
        assert_eq!(comment_id("src/lib.rs", "// keep me", 0), "94596e9edd");
        assert_eq!(comment_id("src/lib.rs", "// keep me", 1), "d057246ce7");
        assert_eq!(
            full_id("src/lib.rs", "// keep me", 0),
            "94596e9eddac2c3b95caedcf688cd705"
        );
        assert_eq!(group_id("keep me"), "cf51d80beb");

        // A full id is the short id widened, not a different hash.
        let (short, full) = (
            comment_id("src/lib.rs", "// keep me", 0),
            full_id("src/lib.rs", "// keep me", 0),
        );
        assert!(full.starts_with(&short), "{full} vs {short}");
    }

    #[test]
    fn inserting_a_line_above_a_comment_does_not_change_its_id() {
        let before = "// keep me\nfn a() {}\n";
        let after = "use std::fmt;\n\n// keep me\nfn a() {}\n";

        assert_eq!(
            ids_for("src/a.rs", &line_comments(before)),
            ids_for("src/a.rs", &line_comments(after)),
        );
    }

    #[test]
    fn reordering_comments_within_a_file_does_not_change_their_ids() {
        let first = ids_for("src/a.rs", &["// alpha", "// beta"]);
        let second = ids_for("src/a.rs", &["// beta", "// alpha"]);
        assert_eq!(first[0], second[1]);
        assert_eq!(first[1], second[0]);
    }

    #[test]
    fn changing_the_comment_text_changes_the_id() {
        assert_ne!(
            comment_id("src/a.rs", "// keep me", 0),
            comment_id("src/a.rs", "// keep me!", 0)
        );
        // Whitespace is part of the exact bytes; normalization belongs to `group_id`.
        assert_ne!(
            comment_id("src/a.rs", "// keep me", 0),
            comment_id("src/a.rs", "//  keep me", 0)
        );
    }

    #[test]
    fn byte_identical_comments_in_one_file_get_distinct_ids() {
        let texts = ["// repeated", "// other", "// repeated", "// repeated"];
        assert_eq!(assign_occurrence_indices(&texts), vec![0, 0, 1, 2]);

        let ids = ids_for("src/a.rs", &texts);
        let unique: BTreeSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
    }

    #[test]
    fn the_same_comment_in_two_files_gets_different_ids() {
        assert_ne!(
            comment_id("src/a.rs", "// keep me", 0),
            comment_id("src/b.rs", "// keep me", 0)
        );
    }

    #[test]
    fn the_field_separators_keep_path_and_text_from_running_together() {
        assert_ne!(comment_id("ab", "c", 0), comment_id("a", "bc", 0));
    }

    #[test]
    fn a_group_id_is_shared_by_instances_that_differ_only_in_layout_and_case() {
        let instances = ["    // Keep Me", "// keep me", "\t//KEEP  ME"];
        let groups: Vec<String> = instances
            .iter()
            .map(|text| group_id(&normalize_comment_text(text)))
            .collect();
        assert_eq!(groups[0], groups[1]);
        assert_eq!(groups[1], groups[2]);

        let ids = ids_for("src/a.rs", &instances);
        let unique: BTreeSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
    }

    #[test]
    fn a_group_id_carries_no_path() {
        let text = normalize_comment_text("// keep me");
        assert_eq!(group_id(&text), group_id(&text));
        assert_ne!(group_id(&text), comment_id("src/a.rs", "// keep me", 0));
    }

    #[test]
    fn a_block_comment_normalizes_to_the_same_string_as_the_equivalent_line_comments() {
        let block = "/*\n * Check the cache first.\n * Fall back to the network.\n */";
        let lines = "// Check the cache first.\n// Fall back to the network.";
        let expected = "check the cache first. fall back to the network.";

        assert_eq!(normalize_comment_text(block), expected);
        assert_eq!(normalize_comment_text(lines), expected);
        assert_eq!(
            group_id(&normalize_comment_text(block)),
            group_id(&normalize_comment_text(lines))
        );
    }

    #[test]
    fn every_delimiter_form_normalizes_to_the_bare_text() {
        let cases = [
            ("// keep me", "keep me"),
            ("/// keep me", "keep me"),
            ("//! keep me", "keep me"),
            ("# keep me", "keep me"),
            ("-- keep me", "keep me"),
            ("/* keep me */", "keep me"),
            ("/** keep me */", "keep me"),
            ("/*! keep me */", "keep me"),
            ("\"\"\" keep me \"\"\"", "keep me"),
            ("''' keep me '''", "keep me"),
            ("\"\"\"\nkeep me\n\"\"\"", "keep me"),
            ("//   keep    me   ", "keep me"),
            ("//\n// keep me\n//", "keep me"),
            ("", ""),
            ("//", ""),
            ("/* */", ""),
        ];

        for (input, expected) in cases {
            assert_eq!(normalize_comment_text(input), expected, "normalizing {input:?}");
        }
    }

    #[test]
    fn normalization_collapses_every_kind_of_whitespace_run() {
        assert_eq!(
            normalize_comment_text("//\tkeep\u{00a0}me \r\n//  again"),
            "keep me again"
        );
    }

    #[test]
    fn normalization_lowercases_non_ascii_deterministically() {
        assert_eq!(normalize_comment_text("// CAFÉ Is Closed"), "café is closed");
    }

    #[test]
    fn a_truncated_collision_is_reported_and_resolved_by_widening() {
        // Two digests sharing their first ID_BYTES bytes, built by hand: brute-forcing a real blake3
        // collision is not a unit test, and the widening path still has to be proven to work.
        let mut left = [0u8; 32];
        for (index, byte) in left.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let mut right = left;
        right[ID_BYTES] = 0xff;

        let short = [truncate_hex(&left, ID_BYTES), truncate_hex(&right, ID_BYTES)];
        assert_eq!(short[0], short[1]);
        assert_eq!(ambiguous_ids(&short), vec![short[0].clone()]);

        let wide = [truncate_hex(&left, FULL_ID_BYTES), truncate_hex(&right, FULL_ID_BYTES)];
        assert_ne!(wide[0], wide[1]);
        assert!(ambiguous_ids(&wide).is_empty());
        assert_eq!(wide[0].len(), FULL_ID_BYTES * 2);
    }

    #[test]
    fn ambiguous_ids_reports_each_colliding_value_once_in_first_seen_order() {
        let ids = ["ccc", "aaa", "bbb", "aaa", "ccc", "aaa"];
        assert_eq!(ambiguous_ids(&ids), vec!["ccc".to_string(), "aaa".to_string()]);
        assert!(ambiguous_ids(&["aaa", "bbb"]).is_empty());
        assert!(ambiguous_ids::<&str>(&[]).is_empty());
    }

    #[test]
    fn truncate_hex_clamps_the_requested_width() {
        let digest = [0xabu8; 32];
        assert_eq!(truncate_hex(&digest, 0).len(), 2);
        assert_eq!(truncate_hex(&digest, 99).len(), 64);
    }

    #[test]
    fn occurrence_indices_are_empty_for_an_empty_file() {
        assert!(assign_occurrence_indices(&[]).is_empty());
    }
}
