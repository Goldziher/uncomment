use crate::language::regex::is_in_string;
use crate::models::language::SupportedLanguage;
use crate::models::line_segment::LineSegment;

/// Process a line with line comments and return segments
pub fn process_line_with_line_comments<'a>(
    line: &'a str,
    comment_marker: &str,
    language: &SupportedLanguage,
) -> (bool, Vec<LineSegment<'a>>) {
    let mut segments = Vec::new();
    let mut found_comment = false;

    let mut pos = 0;
    while let Some(marker_pos) = line[pos..].find(comment_marker) {
        let abs_pos = pos + marker_pos;

        if !is_in_string(line, abs_pos, language) {
            found_comment = true;

            if abs_pos > 0 {
                segments.push(LineSegment::Code(&line[..abs_pos]));
            }

            let comment = &line[abs_pos..];
            segments.push(LineSegment::Comment(comment, comment));
            break;
        }

        pos = abs_pos + comment_marker.len();
    }

    if !found_comment {
        segments.push(LineSegment::Code(line));
    }

    (found_comment, segments)
}

/// Process a line with block comments and return segments
pub fn process_line_with_block_comments<'a>(
    line: &'a str,
    start: &str,
    end: &str,
    language: &SupportedLanguage,
) -> (bool, Vec<LineSegment<'a>>) {
    let mut segments = Vec::new();
    let mut pos = 0;
    let mut found_comment = false;

    while pos < line.len() {
        if let Some(comment_start) = line[pos..].find(start) {
            let abs_start = pos + comment_start;

            if is_in_string(line, abs_start, language) {
                pos = abs_start + start.len();
                continue;
            }

            if abs_start > pos {
                segments.push(LineSegment::Code(&line[pos..abs_start]));
            }

            if let Some(end_pos) = line[abs_start + start.len()..].find(end) {
                let abs_end_pos = abs_start + start.len() + end_pos;

                if !is_in_string(line, abs_end_pos, language) {
                    let abs_end = abs_end_pos + end.len();

                    let comment_content = &line[abs_start + start.len()..abs_end_pos];
                    let full_comment = &line[abs_start..abs_end];

                    segments.push(LineSegment::Comment(comment_content, full_comment));
                    found_comment = true;
                    pos = abs_end;
                } else {
                    segments.push(LineSegment::Code(&line[pos..abs_end_pos]));
                    pos = abs_end_pos;
                }
            } else {
                let comment_content = &line[abs_start + start.len()..];
                let full_comment = &line[abs_start..];

                segments.push(LineSegment::Comment(comment_content, full_comment));
                found_comment = true;
                pos = line.len();
            }
        } else {
            if pos < line.len() {
                segments.push(LineSegment::Code(&line[pos..]));
            }
            break;
        }
    }

    if segments.is_empty() && !line.is_empty() {
        segments.push(LineSegment::Code(line));
    }

    (found_comment, segments)
}
