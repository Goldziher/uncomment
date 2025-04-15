use std::fs;
use std::io;
use std::path::PathBuf;

use crate::language::regex::{find_string_spans, get_or_compile_regexes, is_in_string};
use crate::models::language::SupportedLanguage;
use crate::models::line_segment::LineSegment;
use crate::models::options::ProcessOptions;
use crate::processing::comment::{should_keep_block_comment, should_keep_line_comment};
use crate::processing::line::{process_line_with_block_comments, process_line_with_line_comments};

/// Check if a line contains a real block comment start (not in a string)
pub fn is_real_block_comment_start(line: &str, start: &str, language: &SupportedLanguage) -> bool {
    let mut search_pos = 0;
    while search_pos < line.len() {
        if let Some(pos) = line[search_pos..].find(start) {
            let abs_pos = search_pos + pos;

            if !is_in_string(line, abs_pos, language) {
                return true;
            }

            search_pos = abs_pos + start.len();
        } else {
            break;
        }
    }
    false
}

/// Check if a line has a matching end for a block comment
pub fn has_matching_end(line: &str, start: &str, end: &str, language: &SupportedLanguage) -> bool {
    let mut search_pos = 0;
    while search_pos < line.len() {
        if let Some(start_pos) = line[search_pos..].find(start) {
            let abs_start_pos = search_pos + start_pos;

            if is_in_string(line, abs_start_pos, language) {
                search_pos = abs_start_pos + start.len();
                continue;
            }

            let end_search_start = abs_start_pos + start.len();

            if let Some(end_pos) = line[end_search_start..].find(end) {
                let abs_end_pos = end_search_start + end_pos;

                if !is_in_string(line, abs_end_pos, language) {
                    return true;
                }

                search_pos = abs_end_pos + end.len();
            } else {
                return false;
            }
        } else {
            break;
        }
    }
    false
}

/// Check if a line is inside a string
pub fn is_line_in_string(line: &str, language: &SupportedLanguage) -> bool {
    if line.contains("`") || line.contains("'''") || line.contains("\"\"\"") {
        let backtick_count = line.matches("`").count();
        let triple_single_count = line.matches("'''").count();
        let triple_double_count = line.matches("\"\"\"").count();

        if (language.name == "javascript" || language.name == "typescript")
            && backtick_count % 2 == 1
        {
            return true;
        }

        if language.name == "python"
            && (triple_single_count % 2 == 1 || triple_double_count % 2 == 1)
        {
            return true;
        }
    }

    let spans = find_string_spans(line, language);
    if !spans.is_empty() {
        if line.contains(language.line_comment) {
            let pos = line.find(language.line_comment).unwrap();
            for (start, end) in &spans {
                if pos >= *start && pos < *end {
                    return true;
                }
            }
        }

        if let Some((block_start, block_end)) = language.block_comment {
            if line.contains(block_start) {
                let pos = line.find(block_start).unwrap();
                for (start, end) in &spans {
                    if pos >= *start && pos < *end {
                        return true;
                    }
                }
            }

            if line.contains(block_end) {
                let pos = line.find(block_end).unwrap();
                for (start, end) in &spans {
                    if pos >= *start && pos < *end {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Process a file and remove comments according to the provided options
pub fn process_file(
    file_path: &PathBuf,
    language: &SupportedLanguage,
    options: &ProcessOptions,
) -> io::Result<bool> {
    get_or_compile_regexes(language);

    let file_bytes = fs::read(file_path)?;
    let content = match String::from_utf8(file_bytes) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Warning: File {} contains invalid UTF-8: {}",
                file_path.display(),
                e
            );
            String::from_utf8_lossy(&e.into_bytes()).to_string()
        }
    };

    if content.is_empty() || content.trim().is_empty() {
        if let Some(output_dir) = options.output_dir {
            let output_path = PathBuf::from(output_dir).join(file_path.file_name().unwrap());
            fs::write(&output_path, "\n").unwrap();
        }
        return Ok(true);
    }

    let original_lines: Vec<&str> = content.lines().collect();
    let mut processed_lines: Vec<String> = Vec::with_capacity(original_lines.len());

    let mut in_block_comment = false;
    let mut block_comment_start_line = 0;
    let mut block_comment_text = String::new();

    let mut in_multiline_string = false;
    let mut multiline_string_marker = String::new();

    let mut in_docstring = false;
    let mut skip_next_triple_quote = false;

    for (i, line) in original_lines.iter().enumerate() {
        if language.name == "python" && options.remove_doc {
            let is_func_or_class_start = i > 0
                && original_lines[i - 1].trim().ends_with(":")
                && (original_lines[i - 1].contains("def ")
                    || original_lines[i - 1].contains("class "));

            let trimmed = line.trim();
            let has_triple_quotes = trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''");

            if has_triple_quotes
                && !in_multiline_string
                && !in_block_comment
                && !skip_next_triple_quote
                && (is_func_or_class_start
                    || i < 3
                    || (i > 0
                        && !original_lines[i - 1].trim().is_empty()
                        && line.starts_with(
                            original_lines[i - 1]
                                .chars()
                                .take_while(|c| c.is_whitespace())
                                .collect::<String>()
                                .as_str(),
                        )))
            {
                let is_single_line_triple_quote = (trimmed.ends_with("\"\"\"")
                    && trimmed.starts_with("\"\"\""))
                    || (trimmed.ends_with("'''") && trimmed.starts_with("'''"));

                if !is_single_line_triple_quote {
                    in_docstring = true;
                    if trimmed.starts_with("\"\"\"") {
                        multiline_string_marker = "\"\"\"".to_string();
                    } else {
                        multiline_string_marker = "'''".to_string();
                    }
                    continue;
                } else {
                    skip_next_triple_quote = true;
                    continue;
                }
            } else {
                skip_next_triple_quote = false;
            }
        }

        if in_docstring {
            if line.contains(&multiline_string_marker) {
                in_docstring = false;
            }
            continue;
        }

        if in_multiline_string {
            processed_lines.push(line.to_string());

            if line.contains(&multiline_string_marker) {
                let marker_count = line.matches(&multiline_string_marker).count();
                if marker_count % 2 == 1 {
                    in_multiline_string = false;
                }
            }
            continue;
        }

        let has_string_markers = is_line_in_string(line, language);

        if has_string_markers {
            if line.contains("'''") && line.matches("'''").count() % 2 == 1 {
                in_multiline_string = true;
                multiline_string_marker = "'''".to_string();
            } else if line.contains("\"\"\"") && line.matches("\"\"\"").count() % 2 == 1 {
                in_multiline_string = true;
                multiline_string_marker = "\"\"\"".to_string();
            } else if line.contains("`") && line.matches("`").count() % 2 == 1 {
                in_multiline_string = true;
                multiline_string_marker = "`".to_string();
            }

            processed_lines.push(line.to_string());
            continue;
        }

        if in_block_comment {
            block_comment_text.push_str(line);
            block_comment_text.push('\n');

            if let Some((_, end)) = language.block_comment {
                if line.contains(end) && !is_in_string(line, line.find(end).unwrap(), language) {
                    in_block_comment = false;

                    let should_keep = should_keep_block_comment(
                        &block_comment_text,
                        options.remove_todo,
                        options.remove_fixme,
                        options.remove_doc,
                        options.ignore_patterns,
                        Some(language),
                        options.disable_default_ignores,
                    );

                    if should_keep {
                        for line in original_lines
                            .iter()
                            .skip(block_comment_start_line)
                            .take(i - block_comment_start_line + 1)
                        {
                            processed_lines.push(line.to_string());
                        }
                    } else {
                        for _ in block_comment_start_line..=i {
                            processed_lines.push(String::new());
                        }

                        if let Some(rest) = line.split(end).nth(1) {
                            if !rest.trim().is_empty() {
                                let indent = line
                                    .chars()
                                    .take_while(|c| c.is_whitespace())
                                    .collect::<String>();
                                *processed_lines.last_mut().unwrap() =
                                    format!("{}{}", indent, rest);
                            }
                        }
                    }

                    block_comment_text.clear();
                    continue;
                }
            }

            continue;
        }

        if let Some((start, end)) = language.block_comment {
            if is_real_block_comment_start(line, start, language) {
                if !has_matching_end(line, start, end, language) {
                    in_block_comment = true;
                    block_comment_start_line = i;
                    block_comment_text = line.to_string();
                    block_comment_text.push('\n');
                    continue;
                } else {
                    let should_keep = should_keep_block_comment(
                        line,
                        options.remove_todo,
                        options.remove_fixme,
                        options.remove_doc,
                        options.ignore_patterns,
                        Some(language),
                        options.disable_default_ignores,
                    );

                    if should_keep {
                        processed_lines.push(line.to_string());
                    } else {
                        let (is_comment, segments) =
                            process_line_with_block_comments(line, start, end, language);
                        if is_comment {
                            let mut new_line = String::new();
                            let mut has_code = false;

                            for segment in segments {
                                if let LineSegment::Code(code_text) = segment {
                                    has_code = true;
                                    new_line.push_str(code_text);
                                }
                            }

                            if has_code {
                                processed_lines.push(new_line);
                            } else {
                                processed_lines.push(String::new());
                            }
                        } else {
                            processed_lines.push(line.to_string());
                        }
                    }
                    continue;
                }
            }
        }

        let (is_comment, segments) =
            process_line_with_line_comments(line, language.line_comment, language);
        if is_comment {
            let mut new_line = String::new();
            let mut has_code = false;
            let mut should_keep_comment = false;

            for segment in segments {
                match segment {
                    LineSegment::Comment(comment_text, full_text) => {
                        if should_keep_line_comment(
                            comment_text,
                            options.remove_todo,
                            options.remove_fixme,
                            options.remove_doc,
                            options.ignore_patterns,
                            Some(language),
                            options.disable_default_ignores,
                        ) {
                            should_keep_comment = true;
                            new_line.push_str(full_text);
                        }
                    }
                    LineSegment::Code(code_text) => {
                        has_code = true;
                        new_line.push_str(code_text);
                    }
                }
            }

            if has_code || should_keep_comment {
                processed_lines.push(new_line);
            } else {
                processed_lines.push(String::new());
            }
            continue;
        }

        processed_lines.push(line.to_string());
    }

    let mut result = String::new();

    if content.starts_with('\n') {
        result.push('\n');
    }

    for (i, line) in processed_lines.iter().enumerate() {
        if i > 0 || content.starts_with('\n') {
            result.push('\n');
        }
        result.push_str(line);
    }

    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    } else if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    if result.trim().is_empty() {
        result = String::from("\n");
    }

    let original_content = if content.ends_with('\n') && !original_lines.is_empty() {
        format!("{}\n", original_lines.join("\n"))
    } else {
        original_lines.join("\n")
    };

    let was_modified = original_content != result;

    if was_modified {
        if let Some(output_dir) = options.output_dir {
            let output_path = PathBuf::from(output_dir).join(file_path.file_name().unwrap());
            fs::write(&output_path, &result).unwrap();
        } else if !options.dry_run {
            match fs::write(file_path, &result) {
                Ok(_) => (),
                Err(e) => eprintln!("Error writing to file {}: {}", file_path.display(), e),
            }
        }
    }

    Ok(was_modified)
}
