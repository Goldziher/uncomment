//! Applying many byte-range edits to one file's content in a single pass.
//!
//! Every command that rewrites a file in place funnels through [`apply_edits`], so the ordering and
//! validation rules live in exactly one place: edits are applied back-to-front by descending start
//! offset, which keeps every not-yet-applied offset valid against the original content, and anything
//! that could silently corrupt a file — overlapping ranges, out-of-bounds ranges, offsets landing
//! inside a multi-byte character — is rejected rather than guessed at.

use anyhow::{Result, bail};

/// A replacement of the half-open byte range `[start, end)` in some content.
///
/// `start == end` is an insertion at that offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

impl Edit {
    pub fn new(start: usize, end: usize, replacement: impl Into<String>) -> Self {
        Self {
            start,
            end,
            replacement: replacement.into(),
        }
    }

    /// Deletion of `[start, end)`.
    pub fn delete(start: usize, end: usize) -> Self {
        Self::new(start, end, "")
    }

    /// Insertion at `offset`, replacing nothing.
    pub fn insert(offset: usize, text: impl Into<String>) -> Self {
        Self::new(offset, offset, text)
    }
}

/// Apply every edit to `content`, returning the new text.
///
/// Fails if any range is reversed, reaches past the end of `content`, splits a UTF-8 character, or
/// overlaps another edit. Two ranges that merely touch (`a.end == b.start`) do not overlap, and an
/// empty edit list returns `content` unchanged.
pub fn apply_edits(content: &str, edits: Vec<Edit>) -> Result<String> {
    if edits.is_empty() {
        return Ok(content.to_string());
    }

    let mut edits = edits;
    for edit in &edits {
        validate_range(content, edit)?;
    }

    // Ascending by (start, end) so a zero-width insertion sorts ahead of a replacement beginning at
    // the same offset; that ordering is what makes the adjacent-pair overlap sweep below exact, and
    // reversing it yields the descending order the edits are applied in.
    edits.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    reject_overlaps(&edits)?;

    let mut out = content.to_string();
    for edit in edits.iter().rev() {
        out.replace_range(edit.start..edit.end, &edit.replacement);
    }
    Ok(out)
}

fn validate_range(content: &str, edit: &Edit) -> Result<()> {
    let len = content.len();
    if edit.end < edit.start {
        bail!("invalid edit range {}..{}: end precedes start", edit.start, edit.end);
    }
    if edit.end > len {
        bail!(
            "edit range {}..{} is out of bounds for content of {} bytes",
            edit.start,
            edit.end,
            len
        );
    }
    if !content.is_char_boundary(edit.start) {
        bail!(
            "edit range {}..{} starts inside a multi-byte character at byte {}",
            edit.start,
            edit.end,
            edit.start
        );
    }
    if !content.is_char_boundary(edit.end) {
        bail!(
            "edit range {}..{} ends inside a multi-byte character at byte {}",
            edit.start,
            edit.end,
            edit.end
        );
    }
    Ok(())
}

/// Reject overlapping ranges in a slice already sorted ascending by `(start, end)`.
///
/// Sorted that way the immediate predecessor is enough to decide the next range: a range whose
/// start falls strictly before the predecessor's end intersects it, and equality means they only
/// touch. No earlier range needs consulting, because starts are non-decreasing, so once every
/// adjacent pair up to `i` has passed, `edits[i].start >= edits[j + 1].start >= edits[j].end` holds
/// for every earlier `j`. A short range nested inside a long one is still caught: the long range is
/// its predecessor in this order, and the pair fails there.
fn reject_overlaps(edits: &[Edit]) -> Result<()> {
    let mut previous = &edits[0];
    for edit in &edits[1..] {
        if edit.start < previous.end {
            bail!(
                "overlapping edits: {}..{} overlaps {}..{}",
                previous.start,
                previous.end,
                edit.start,
                edit.end
            );
        }
        previous = edit;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_edit_replaces_the_named_range() {
        let out = apply_edits("hello world", vec![Edit::new(6, 11, "rust")]).unwrap();
        assert_eq!(out, "hello rust");
    }

    #[test]
    fn several_edits_apply_in_one_pass_with_correct_offsets() {
        // Offsets are all against the original content; a replacement that changes length must not
        // shift the ones that follow it.
        let content = "alpha beta gamma delta";
        let out = apply_edits(
            content,
            vec![
                Edit::new(0, 5, "a-much-longer-word"),
                Edit::new(6, 10, "B"),
                Edit::delete(11, 17),
                Edit::new(17, 22, "DELTA"),
            ],
        )
        .unwrap();
        assert_eq!(out, "a-much-longer-word B DELTA");
    }

    #[test]
    fn edits_given_out_of_order_still_apply_correctly() {
        let out = apply_edits(
            "one two three",
            vec![
                Edit::new(8, 13, "THREE"),
                Edit::new(0, 3, "ONE"),
                Edit::new(4, 7, "TWO"),
            ],
        )
        .unwrap();
        assert_eq!(out, "ONE TWO THREE");
    }

    #[test]
    fn zero_width_edits_insert_at_the_start_and_the_end() {
        assert_eq!(apply_edits("body", vec![Edit::insert(0, ">> ")]).unwrap(), ">> body");
        assert_eq!(apply_edits("body", vec![Edit::insert(4, " <<")]).unwrap(), "body <<");
        assert_eq!(
            apply_edits("body", vec![Edit::insert(0, ">> "), Edit::insert(4, " <<")]).unwrap(),
            ">> body <<"
        );
    }

    #[test]
    fn insertion_into_empty_content_is_allowed() {
        assert_eq!(apply_edits("", vec![Edit::insert(0, "new")]).unwrap(), "new");
    }

    #[test]
    fn two_insertions_at_one_offset_keep_their_given_order() {
        let out = apply_edits("xy", vec![Edit::insert(1, "A"), Edit::insert(1, "B")]).unwrap();
        assert_eq!(out, "xABy");
    }

    #[test]
    fn touching_ranges_are_not_overlapping() {
        let out = apply_edits("abcdef", vec![Edit::new(0, 3, "1"), Edit::new(3, 6, "2")]).unwrap();
        assert_eq!(out, "12");
    }

    #[test]
    fn overlapping_edits_are_rejected_naming_both_ranges() {
        let err = apply_edits("abcdefghij", vec![Edit::new(0, 5, "x"), Edit::new(3, 8, "y")]).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("0..5"), "{message}");
        assert!(message.contains("3..8"), "{message}");
    }

    #[test]
    fn a_range_nested_inside_another_is_rejected() {
        // 2..3 sits inside 0..9 and follows it directly once sorted, so the sweep rejects that pair
        // before it ever reaches 6..7.
        let err = apply_edits(
            "abcdefghij",
            vec![Edit::new(0, 9, "x"), Edit::new(2, 3, "y"), Edit::new(6, 7, "z")],
        )
        .unwrap_err();
        assert!(err.to_string().contains("overlapping edits"), "{err}");
    }

    #[test]
    fn an_insertion_inside_another_edits_range_is_rejected() {
        let err = apply_edits("abcdefghij", vec![Edit::new(0, 5, "x"), Edit::insert(2, "y")]).unwrap_err();
        assert!(err.to_string().contains("overlapping edits"), "{err}");
    }

    #[test]
    fn an_insertion_at_the_boundary_of_another_edit_is_allowed() {
        assert_eq!(
            apply_edits("abcdef", vec![Edit::new(2, 4, "X"), Edit::insert(2, "<")]).unwrap(),
            "ab<Xef"
        );
        assert_eq!(
            apply_edits("abcdef", vec![Edit::new(2, 4, "X"), Edit::insert(4, ">")]).unwrap(),
            "abX>ef"
        );
    }

    #[test]
    fn identical_ranges_are_rejected_rather_than_last_one_wins() {
        let err = apply_edits("abcdef", vec![Edit::new(1, 4, "x"), Edit::new(1, 4, "y")]).unwrap_err();
        assert!(err.to_string().contains("overlapping edits"), "{err}");
    }

    #[test]
    fn out_of_bounds_ranges_are_rejected() {
        let err = apply_edits("abc", vec![Edit::new(1, 9, "x")]).unwrap_err();
        assert!(err.to_string().contains("out of bounds"), "{err}");

        let err = apply_edits("abc", vec![Edit::insert(4, "x")]).unwrap_err();
        assert!(err.to_string().contains("out of bounds"), "{err}");
    }

    #[test]
    fn reversed_ranges_are_rejected() {
        let err = apply_edits("abcdef", vec![Edit::new(4, 2, "x")]).unwrap_err();
        assert!(err.to_string().contains("end precedes start"), "{err}");
    }

    #[test]
    fn offsets_inside_a_multi_byte_character_are_rejected() {
        let content = "// café\ncode();\n";
        let e_acute = content.find('é').unwrap();
        assert!(!content.is_char_boundary(e_acute + 1));

        let err = apply_edits(content, vec![Edit::new(0, e_acute + 1, "")]).unwrap_err();
        assert!(err.to_string().contains("ends inside a multi-byte character"), "{err}");

        let err = apply_edits(content, vec![Edit::new(e_acute + 1, content.len(), "")]).unwrap_err();
        assert!(
            err.to_string().contains("starts inside a multi-byte character"),
            "{err}"
        );
    }

    #[test]
    fn multi_byte_content_is_edited_correctly_on_character_boundaries() {
        let content = "# 日本語\nvalue = 1\n";
        let comment_end = content.find('\n').unwrap();
        let out = apply_edits(content, vec![Edit::delete(0, comment_end + 1)]).unwrap();
        assert_eq!(out, "value = 1\n");

        let out = apply_edits(content, vec![Edit::new(2, comment_end, "コメント")]).unwrap();
        assert_eq!(out, "# コメント\nvalue = 1\n");
    }

    #[test]
    fn an_empty_edit_list_is_a_no_op_and_stays_one_when_repeated() {
        let content = "unchanged // café\n";
        let once = apply_edits(content, Vec::new()).unwrap();
        let twice = apply_edits(&once, Vec::new()).unwrap();
        assert_eq!(once, content);
        assert_eq!(twice, content);
    }

    #[test]
    fn every_triple_of_ranges_is_judged_the_way_pairwise_intersection_says() {
        // The sweep in `reject_overlaps` leans on the sort order to look at one pair of ranges per
        // step. This pins its verdict to the definition it is supposed to implement — two half-open
        // ranges intersect when each starts strictly before the other ends, which is also what makes
        // a zero-width insertion at a boundary fine and one in mid-range not — over every triple of
        // ranges in a four-byte content, rather than over the handful spelled out above.
        const CONTENT: &str = "abcd";

        let ranges: Vec<(usize, usize)> = (0..=CONTENT.len())
            .flat_map(|start| (start..=CONTENT.len()).map(move |end| (start, end)))
            .collect();
        let intersects = |a: (usize, usize), b: (usize, usize)| a.0 < b.1 && b.0 < a.1;

        for &a in &ranges {
            for &b in &ranges {
                for &c in &ranges {
                    let expected_overlap = intersects(a, b) || intersects(a, c) || intersects(b, c);
                    let edits = vec![
                        Edit::new(a.0, a.1, "A"),
                        Edit::new(b.0, b.1, "B"),
                        Edit::new(c.0, c.1, "C"),
                    ];
                    let accepted = apply_edits(CONTENT, edits).is_ok();
                    assert_eq!(
                        accepted, !expected_overlap,
                        "{a:?} {b:?} {c:?}: accepted = {accepted}, intersecting = {expected_overlap}"
                    );
                }
            }
        }
    }
}
