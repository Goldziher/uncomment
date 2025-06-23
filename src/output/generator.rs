use crate::ast::visitor::CommentInfo;
use anyhow::Result;

#[allow(dead_code)]
pub struct OutputGenerator<'a> {
    source: &'a str,
}

#[allow(dead_code)]
impl<'a> OutputGenerator<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub fn generate_output(&self, comments_to_remove: &[CommentInfo]) -> Result<String> {
        if comments_to_remove.is_empty() {
            return Ok(self.source.to_string());
        }

        // Sort comments by their byte position in reverse order
        // This allows us to remove them from the end to the beginning
        // without affecting the byte positions of earlier comments
        let mut sorted_comments = comments_to_remove.to_vec();
        sorted_comments.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));

        let mut result = self.source.to_string();

        for comment in sorted_comments {
            result = self.remove_comment_from_string(&result, &comment)?;
        }

        // Clean up excessive whitespace that might be left after comment removal
        self.cleanup_whitespace(&result)
    }

    fn remove_comment_from_string(&self, content: &str, comment: &CommentInfo) -> Result<String> {
        let start = comment.start_byte;
        let end = comment.end_byte;

        if start > content.len() || end > content.len() || start > end {
            return Err(anyhow::anyhow!("Invalid comment byte range"));
        }

        // Find the start and end of the line containing the comment
        let (line_start, line_end) = self.find_line_boundaries(content, start, end);

        // Check if the comment is the only content on its line(s)
        let before_comment = &content[line_start..start];
        let after_comment = &content[end..line_end];

        let only_whitespace_before = before_comment.trim().is_empty();
        let only_whitespace_after = after_comment.trim().is_empty();

        if only_whitespace_before && only_whitespace_after {
            // Remove the entire line(s) including newlines
            let remove_start = line_start;
            let mut remove_end = line_end;

            // Include the newline character if present
            if remove_end < content.len() && content.chars().nth(remove_end) == Some('\n') {
                remove_end += 1;
            }

            Ok(format!(
                "{}{}",
                &content[..remove_start],
                &content[remove_end..]
            ))
        } else {
            // Remove only the comment, preserving other content on the line
            Ok(format!("{}{}", &content[..start], &content[end..]))
        }
    }

    fn find_line_boundaries(&self, content: &str, start: usize, end: usize) -> (usize, usize) {
        // Find the start of the line containing the comment
        let line_start = content[..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);

        // Find the end of the line containing the comment
        let line_end = content[end..]
            .find('\n')
            .map(|pos| end + pos)
            .unwrap_or(content.len());

        (line_start, line_end)
    }

    fn cleanup_whitespace(&self, content: &str) -> Result<String> {
        let lines: Vec<&str> = content.lines().collect();
        let mut result_lines = Vec::new();
        let mut consecutive_empty = 0;

        for line in lines {
            if line.trim().is_empty() {
                consecutive_empty += 1;
                // Limit consecutive empty lines to 2
                if consecutive_empty <= 2 {
                    result_lines.push(line);
                }
            } else {
                consecutive_empty = 0;
                result_lines.push(line);
            }
        }

        // Remove trailing empty lines, but keep at most one
        while result_lines.len() > 1 && result_lines.last().unwrap().trim().is_empty() {
            if result_lines
                .get(result_lines.len() - 2)
                .map(|line| line.trim().is_empty())
                .unwrap_or(false)
            {
                result_lines.pop();
            } else {
                break;
            }
        }

        Ok(result_lines.join("\n"))
    }

    pub fn preview_changes(&self, comments_to_remove: &[CommentInfo]) -> Result<String> {
        let mut preview = String::new();
        preview.push_str("Comments to be removed:\n");
        preview.push_str("======================\n\n");

        for (i, comment) in comments_to_remove.iter().enumerate() {
            preview.push_str(&format!(
                "{}. Line {}: {}\n",
                i + 1,
                comment.start_row + 1, // Convert to 1-based line numbers
                comment.content.trim()
            ));
        }

        preview.push_str(&format!(
            "\nTotal comments to remove: {}\n",
            comments_to_remove.len()
        ));

        Ok(preview)
    }

    pub fn generate_diff(&self, comments_to_remove: &[CommentInfo]) -> Result<String> {
        let original_lines: Vec<&str> = self.source.lines().collect();
        let processed_content = self.generate_output(comments_to_remove)?;
        let processed_lines: Vec<&str> = processed_content.lines().collect();

        let mut diff = String::new();
        diff.push_str("--- Original\n");
        diff.push_str("+++ Processed\n");

        // Simple diff: show all lines from original, then all from processed
        // In a real implementation, you'd use a proper diff algorithm like Myers
        let max_lines = original_lines.len().max(processed_lines.len());

        for i in 0..max_lines {
            match (original_lines.get(i), processed_lines.get(i)) {
                (Some(orig_line), Some(proc_line)) => {
                    if orig_line == proc_line {
                        diff.push_str(&format!("  {}\n", orig_line));
                    } else {
                        diff.push_str(&format!("- {}\n", orig_line));
                        diff.push_str(&format!("+ {}\n", proc_line));
                    }
                }
                (Some(orig_line), None) => {
                    diff.push_str(&format!("- {}\n", orig_line));
                }
                (None, Some(proc_line)) => {
                    diff.push_str(&format!("+ {}\n", proc_line));
                }
                (None, None) => break,
            }
        }

        Ok(diff)
    }

    pub fn get_statistics(&self, comments_to_remove: &[CommentInfo]) -> OutputStatistics {
        let original_lines = self.source.lines().count();
        let original_bytes = self.source.len();

        let processed_content = self
            .generate_output(comments_to_remove)
            .unwrap_or_else(|_| self.source.to_string());
        let processed_lines = processed_content.lines().count();
        let processed_bytes = processed_content.len();

        OutputStatistics {
            original_lines,
            processed_lines,
            original_bytes,
            processed_bytes,
            comments_removed: comments_to_remove.len(),
            lines_removed: original_lines.saturating_sub(processed_lines),
            bytes_saved: original_bytes.saturating_sub(processed_bytes),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OutputStatistics {
    pub original_lines: usize,
    pub processed_lines: usize,
    pub original_bytes: usize,
    pub processed_bytes: usize,
    pub comments_removed: usize,
    pub lines_removed: usize,
    pub bytes_saved: usize,
}

#[allow(dead_code)]
impl OutputStatistics {
    pub fn reduction_percentage(&self) -> f64 {
        if self.original_bytes == 0 {
            0.0
        } else {
            (self.bytes_saved as f64 / self.original_bytes as f64) * 100.0
        }
    }

    pub fn lines_reduction_percentage(&self) -> f64 {
        if self.original_lines == 0 {
            0.0
        } else {
            (self.lines_removed as f64 / self.original_lines as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_comment(start: usize, end: usize, content: &str) -> CommentInfo {
        CommentInfo {
            start_byte: start,
            end_byte: end,
            start_row: 0,
            end_row: 0,
            content: content.to_string(),
            node_type: "comment".to_string(),
            should_preserve: false,
        }
    }

    #[test]
    fn test_simple_comment_removal() {
        let source = "// Comment\nfn main() {}";
        let generator = OutputGenerator::new(source);
        let comments = vec![create_test_comment(0, 10, "// Comment")];

        let result = generator.generate_output(&comments).unwrap();
        assert_eq!(result, "fn main() {}");
    }

    #[test]
    fn test_no_comments_to_remove() {
        let source = "fn main() {}";
        let generator = OutputGenerator::new(source);
        let comments = vec![];

        let result = generator.generate_output(&comments).unwrap();
        assert_eq!(result, source);
    }

    #[test]
    fn test_multiple_comments() {
        let source =
            "// First comment\nfn main() {\n    // Second comment\n    println!(\"Hello\");\n}";
        let generator = OutputGenerator::new(source);
        let comments = vec![
            create_test_comment(0, 16, "// First comment"),
            create_test_comment(30, 47, "    // Second comment"),
        ];

        let result = generator.generate_output(&comments).unwrap();
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!(\"Hello\");"));
        assert!(!result.contains("First comment"));
        assert!(!result.contains("Second comment"));
    }

    #[test]
    fn test_inline_comment_removal() {
        let source = "let x = 5; // inline comment\nlet y = 10;";
        let generator = OutputGenerator::new(source);
        let comments = vec![create_test_comment(11, 28, "// inline comment")];

        let result = generator.generate_output(&comments).unwrap();
        assert!(result.contains("let x = 5;"));
        assert!(result.contains("let y = 10;"));
        assert!(!result.contains("inline comment"));
    }

    #[test]
    fn test_find_line_boundaries() {
        let source = "line1\nline2\nline3";
        let generator = OutputGenerator::new(source);

        // Test comment in the middle of line2
        let (start, end) = generator.find_line_boundaries(source, 8, 10);
        assert_eq!(start, 6); // Start of "line2"
        assert_eq!(end, 11); // End of "line2"

        // Test comment at the beginning
        let (start, end) = generator.find_line_boundaries(source, 0, 3);
        assert_eq!(start, 0);
        assert_eq!(end, 5);
    }

    #[test]
    fn test_cleanup_whitespace() {
        let source = "line1\n\n\n\nline2\n\n\n";
        let generator = OutputGenerator::new(source);

        let result = generator.cleanup_whitespace(source).unwrap();
        let empty_line_count = result.matches("\n\n").count();
        assert!(empty_line_count <= 2); // Should limit consecutive empty lines
    }

    #[test]
    fn test_preview_changes() {
        let source = "// Comment 1\n// Comment 2\nfn main() {}";
        let generator = OutputGenerator::new(source);
        let comments = vec![
            create_test_comment(0, 12, "// Comment 1"),
            create_test_comment(13, 25, "// Comment 2"),
        ];

        let preview = generator.preview_changes(&comments).unwrap();
        assert!(preview.contains("Comments to be removed:"));
        assert!(preview.contains("Comment 1"));
        assert!(preview.contains("Comment 2"));
        assert!(preview.contains("Total comments to remove: 2"));
    }

    #[test]
    fn test_generate_diff() {
        let source = "// Comment\nfn main() {}";
        let generator = OutputGenerator::new(source);
        let comments = vec![create_test_comment(0, 11, "// Comment\n")]; // Include newline in comment

        let diff = generator.generate_diff(&comments).unwrap();
        assert!(diff.contains("--- Original"));
        assert!(diff.contains("+++ Processed"));
        assert!(diff.contains("- // Comment"));
        // The function should still appear in the output since it wasn't removed
        assert!(diff.contains("fn main()"));
    }

    #[test]
    fn test_statistics() {
        let source = "// Comment\nfn main() {}";
        let generator = OutputGenerator::new(source);
        let comments = vec![create_test_comment(0, 11, "// Comment\n")];

        let stats = generator.get_statistics(&comments);
        assert_eq!(stats.comments_removed, 1);
        assert!(stats.bytes_saved > 0);
        assert!(stats.reduction_percentage() > 0.0);
    }

    #[test]
    fn test_output_statistics() {
        let stats = OutputStatistics {
            original_lines: 100,
            processed_lines: 80,
            original_bytes: 1000,
            processed_bytes: 800,
            comments_removed: 20,
            lines_removed: 20,
            bytes_saved: 200,
        };

        assert_eq!(stats.reduction_percentage(), 20.0);
        assert_eq!(stats.lines_reduction_percentage(), 20.0);
    }

    #[test]
    fn test_edge_cases() {
        let generator = OutputGenerator::new("");
        let result = generator.generate_output(&[]).unwrap();
        assert_eq!(result, "");

        // Test with invalid byte ranges (should not panic)
        let source = "test";
        let generator = OutputGenerator::new(source);
        let invalid_comment = create_test_comment(10, 20, "invalid");
        let result = generator.generate_output(&[invalid_comment]);
        assert!(result.is_err());
    }
}
