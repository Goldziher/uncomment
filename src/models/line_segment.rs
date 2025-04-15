/// Represents a segment of a line of code, which can be either a comment or code
#[derive(Debug)]
pub enum LineSegment<'a> {
    /// A comment segment with the original text and the comment content
    Comment(&'a str, &'a str),
    /// A code segment with the original text
    Code(&'a str),
}
