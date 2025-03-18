use clap::Parser;
use glob::glob;
use regex::{Regex, RegexSet};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Hash, Eq, PartialEq, Debug)]
struct SupportedLanguage {
    name: &'static str,
    line_comment: &'static str,
    block_comment: Option<(&'static str, &'static str)>,
    doc_string: Option<(&'static str, &'static str)>,
    default_ignore_patterns: Vec<&'static str>,
    string_regex_patterns: Vec<&'static str>,
}

impl SupportedLanguage {
    fn new(
        name: &'static str,
        line_comment: &'static str,
        block_comment: Option<(&'static str, &'static str)>,
        doc_string: Option<(&'static str, &'static str)>,
        default_ignore_patterns: Vec<&'static str>,
        string_regex_patterns: Vec<&'static str>,
    ) -> Self {
        Self {
            name,
            line_comment,
            block_comment,
            doc_string,
            default_ignore_patterns,
            string_regex_patterns,
        }
    }
}

impl Clone for SupportedLanguage {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            line_comment: self.line_comment,
            block_comment: self.block_comment,
            doc_string: self.doc_string,
            default_ignore_patterns: self.default_ignore_patterns.clone(),
            string_regex_patterns: self.string_regex_patterns.clone(),
        }
    }
}

enum LineSegment<'a> {
    Comment(&'a str, &'a str),
    Code(&'a str),
}

struct ProcessOptions<'a> {
    remove_todo: bool,
    remove_fixme: bool,
    remove_doc: bool,
    ignore_patterns: &'a Option<Vec<String>>,
    output_dir: &'a Option<String>,
    disable_default_ignores: bool,
    dry_run: bool,
}

#[derive(Clone)]
struct CompiledRegexes {
    regex_set: RegexSet,
    regex_patterns: Vec<Regex>,
    string_spans: HashMap<String, Vec<(usize, usize)>>,
}

static COMPILED_REGEXES: OnceLock<HashMap<&'static str, CompiledRegexes>> = OnceLock::new();

fn get_or_compile_regexes(language: &SupportedLanguage) -> &CompiledRegexes {
    let regexes_map = COMPILED_REGEXES.get_or_init(initialize_regex_patterns);

    regexes_map.get(language.name).unwrap_or_else(|| {
        panic!("Language {} not initialized in regex cache", language.name);
    })
}

fn find_string_spans(line: &str, language: &SupportedLanguage) -> Vec<(usize, usize)> {
    let line_key = format!("{}:{}", language.name, line);

    let regexes = get_or_compile_regexes(language);
    if let Some(spans) = regexes.string_spans.get(&line_key) {
        return spans.clone();
    }

    let mut spans = Vec::new();
    let matches = regexes.regex_set.matches(line);

    for match_idx in matches.iter() {
        if match_idx >= regexes.regex_patterns.len() {
            continue;
        }

        let pattern = &regexes.regex_patterns[match_idx];
        for pattern_match in pattern.find_iter(line) {
            spans.push((pattern_match.start(), pattern_match.end()));
        }
    }

    spans.sort_by_key(|span| span.0);

    let mut merged_spans: Vec<(usize, usize)> = Vec::new();
    for span in spans {
        if let Some(last) = merged_spans.last_mut() {
            if span.0 <= last.1 {
                *last = (last.0, span.1.max(last.1));
                continue;
            }
        }
        merged_spans.push(span);
    }

    merged_spans
}

fn is_in_string(line: &str, pos: usize, language: &SupportedLanguage) -> bool {
    if pos >= line.len() {
        return false;
    }

    let spans = find_string_spans(line, language);
    for (start, end) in spans {
        if pos >= start && pos < end {
            return true;
        }
    }

    false
}

fn process_line_with_line_comments<'a>(
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

fn process_line_with_block_comments<'a>(
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

fn is_real_block_comment_start(line: &str, start: &str, language: &SupportedLanguage) -> bool {
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

fn has_matching_end(line: &str, start: &str, end: &str, language: &SupportedLanguage) -> bool {
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

fn initialize_regex_patterns() -> HashMap<&'static str, CompiledRegexes> {
    let mut map = HashMap::new();

    for language in get_supported_languages() {
        let patterns = language.string_regex_patterns.clone();

        let regex_set = RegexSet::new(&patterns).unwrap_or_else(|e| {
            panic!("Invalid regex pattern for {}: {}", language.name, e);
        });

        let regex_patterns = patterns
            .iter()
            .map(|p| {
                Regex::new(p).unwrap_or_else(|e| {
                    panic!("Invalid regex pattern for {}: {} - {}", language.name, p, e);
                })
            })
            .collect();

        map.insert(
            language.name,
            CompiledRegexes {
                regex_set,
                regex_patterns,
                string_spans: HashMap::new(),
            },
        );
    }

    map
}

fn get_supported_languages() -> HashSet<SupportedLanguage> {
    let mut languages = HashSet::new();

    languages.insert(SupportedLanguage::new(
        "rust",
        "//",
        Some(("/*", "*/")),
        Some(("///", "\n")),
        vec![
            "#[", "allow(", "cfg_attr", "deny(", "forbid(", "warn(", "expect(", "cfg(", "#![",
        ],
        vec![
            r#""(?:\\.|[^"\\])*""#,
            r#"r"(?:\\.|[^"\\])*""#,
            r#"b"(?:\\.|[^"\\])*""#,
            r#"br"(?:\\.|[^"\\])*""#,
            r#"'(?:\\[nrt0\\']|\\x[0-9a-fA-F]{2}|\\u\{[0-9a-fA-F]{1,6}\}|[^\\'])'"#,
            r#"b'(?:\\[nrt0\\']|\\x[0-9a-fA-F]{2}|[^\\'])'"#,
            r#"b"(?:\\.|[^"\\])*""#,
            "r#\"[^\"]*\"#",
            "r##\"[^\"]*\"##",
            "r###\"[^\"]*\"###",
            "br#\"[^\"]*\"#",
            "br##\"[^\"]*\"##",
            "br###\"[^\"]*\"###",
        ],
    ));

    languages.insert(SupportedLanguage::new(
        "c",
        "//",
        Some(("/*", "*/")),
        None,
        vec![
            "#pragma", "#include", "#ifdef", "#ifndef", "#define", "#endif", "#if", "#else",
            "#elif",
        ],
        vec![r#""(?:\\.|[^"\\])*""#, r#"'(?:\\.|[^'\\])'"#],
    ));

    languages.insert(SupportedLanguage::new(
        "cpp",
        "//",
        Some(("/*", "*/")),
        None,
        vec![
            "#pragma", "#include", "#ifdef", "#ifndef", "#define", "#endif", "#if", "#else",
            "#elif",
        ],
        vec![
            r#""(?:\\.|[^"\\])*""#,
            r#"'(?:\\.|[^'\\])'"#,
            r#"R"(\(.*\))""#,
            r#"""""(?:[^"]|"[^"]|""[^"])*""""#,
        ],
    ));

    languages.insert(SupportedLanguage::new(
        "java",
        "//",
        Some(("/*", "*/")),
        Some(("/**", "*/")),
        vec![
            "@Override",
            "@Deprecated",
            "@SuppressWarnings",
            "@Nullable",
            "@NonNull",
            "@Generated",
        ],
        vec![
            r#""(?:\\.|[^"\\])*""#,
            r#"'(?:\\.|[^'\\])'"#,
            r#"""""(?:[^"]|"[^"]|""[^"])*""""#,
        ],
    ));

    languages.insert(SupportedLanguage::new(
        "javascript",
        "//",
        Some(("/*", "*/")),
        Some(("/**", "*/")),
        vec![
            "@flow",
            "@ts-ignore",
            "@ts-nocheck",
            "@ts-check",
            "eslint-disable",
            "eslint-enable",
            "eslint-disable-next-line",
            "prettier-ignore",
            "@jsx",
            "@license",
            "@preserve",
        ],
        vec![
            r#""(?:\\.|[^"\\])*""#,
            r#"'(?:\\.|[^'\\])*'"#,
            r#"`[^`]*`"#,
            r#"`[^`]*\$\{[^\}]*\}[^`]*`"#,
        ],
    ));

    languages.insert(SupportedLanguage::new(
        "typescript",
        "//",
        Some(("/*", "*/")),
        Some(("/**", "*/")),
        vec![
            "@ts-ignore",
            "@ts-nocheck",
            "@ts-check",
            "eslint-disable",
            "eslint-enable",
            "eslint-disable-next-line",
            "prettier-ignore",
            "@jsx",
            "@license",
            "@preserve",
        ],
        vec![
            r#""(?:\\.|[^"\\])*""#,
            r#"'(?:\\.|[^'\\])*'"#,
            r#"`[^`]*`"#,
            r#"`[^`]*\$\{[^\}]*\}[^`]*`"#,
        ],
    ));

    languages.insert(SupportedLanguage::new(
        "python",
        "#",
        Some(("'''", "'''")),
        Some(("\"\"\"", "\"\"\"")),
        vec![
            "# noqa",
            "# type:",
            "# pragma:",
            "# pylint:",
            "# mypy:",
            "# ruff:",
            "# flake8:",
            "# fmt:",
            "# isort:",
            "# FIXME:",
            "# TODO:",
            "# NOTE:",
            "# Ignore",
            "# pyright:",
        ],
        vec![
            r#"'(?:\\.|[^'\\])*'"#,
            r#""(?:\\.|[^"\\])*""#,
            r#"'''(?:\\.|[^'\\]|'[^']|''[^'])*'''"#,
            r#"""""(?:\\.|[^"\\]|"[^"]|""[^"])*""""#,
            r#"r'(?:\\.|[^'\\])*'"#,
            r#"r"(?:\\.|[^"\\])*""#,
            r#"r'''(?:\\.|[^'\\]|'[^']|''[^'])*'''"#,
            r#"r"""(?:\\.|[^"\\]|"[^"]|""[^"])*""""#,
            r#"f'[^']*'"#,
            r#"f'[^']*\{[^\}]*\}[^']*'"#,
            r#"f"[^"]*""#,
            r#"f"[^"]*\{[^\}]*\}[^"]*""#,
            r#"f'''[^']*'''"#,
            r#"f"""[^"]*""""#,
            r#"b'(?:\\.|[^'\\])*'"#,
            r#"b"(?:\\.|[^"\\])*""#,
            r#"b'''(?:\\.|[^'\\]|'[^']|''[^'])*'''"#,
            r#"b"""(?:\\.|[^"\\]|"[^"]|""[^"])*""""#,
            r#"u'(?:\\.|[^'\\])*'"#,
            r#"u"(?:\\.|[^"\\])*""#,
            r#"fr'[^']*'"#,
            r#"rf'[^']*'"#,
            r#"fr'[^']*\{[^\}]*\}[^']*'"#,
            r#"rf'[^']*\{[^\}]*\}[^']*'"#,
            r#"fr"[^"]*""#,
            r#"rf"[^"]*""#,
            r#"fr"[^"]*\{[^\}]*\}[^"]*""#,
            r#"rf"[^"]*\{[^\}]*\}[^"]*""#,
            r#"br'(?:\\.|[^'\\])*'"#,
            r#"rb'(?:\\.|[^'\\])*'"#,
            r#"br"(?:\\.|[^"\\])*""#,
            r#"rb"(?:\\.|[^"\\])*""#,
            r#"ur'(?:\\.|[^'\\])*'"#,
            r#"ru'(?:\\.|[^'\\])*'"#,
            r#"ur"(?:\\.|[^"\\])*""#,
            r#"ru"(?:\\.|[^"\\])*""#,
        ],
    ));

    languages.insert(SupportedLanguage::new(
        "ruby",
        "#",
        Some(("=begin", "=end")),
        None,
        vec![
            "# rubocop:disable",
            "# rubocop:enable",
            "# frozen_string_literal:",
        ],
        vec![
            r#"'(?:\\.|[^'\\])*'"#,
            r#""(?:\\.|[^"\\])*""#,
            r#"%q\{[^\}]*\}"#,
            r#"%Q\{[^\}]*\}"#,
            r#"%s\{[^\}]*\}"#,
            r#"%w\{[^\}]*\}"#,
            r#"%W\{[^\}]*\}"#,
            r#"%i\{[^\}]*\}"#,
            r#"%q\([^\)]*\)"#,
            r#"%Q\([^\)]*\)"#,
            r#"%s\([^\)]*\)"#,
            r#"%w\([^\)]*\)"#,
            r#"%W\([^\)]*\)"#,
            r#"%i\([^\)]*\)"#,
            r#"%q\[[^\]]*\]"#,
            r#"%Q\[[^\]]*\]"#,
            r#"%s\[[^\]]*\]"#,
            r#"%w\[[^\]]*\]"#,
            r#"%W\[[^\]]*\]"#,
            r#"%i\[[^\]]*\]"#,
            r#"%r\{[^\}]*\}"#,
            r#"%r\{[^\}]*\}i"#,
            r#"%r\{[^\}]*\}o"#,
            r#"%r\{[^\}]*\}m"#,
            r#"%r\{[^\}]*\}x"#,
            r#"<<[-~]?'\w+'"#,
            r#"<<[-~]?\"\w+\""#,
            r#"<<[-~]?`\w+`"#,
            r#"<<[-~]?\w+"#,
        ],
    ));

    languages.insert(SupportedLanguage::new(
        "go",
        "//",
        Some(("/*", "*/")),
        None,
        vec![
            "//go:build",
            "//go:generate",
            "//nolint",
            "//lint:ignore",
            "//noinspection",
        ],
        vec![r#""(?:\\.|[^"\\])*""#, r#"`[^`]*`"#, r#"'(?:\\.|[^'\\])'"#],
    ));

    languages.insert(SupportedLanguage::new(
        "swift",
        "//",
        Some(("/*", "*/")),
        None,
        vec![
            "// swiftlint:disable",
            "// swiftlint:enable",
            "// MARK:",
            "// sourcery:",
        ],
        vec![
            r#""(?:\\.|[^"\\])*""#,
            r#"""""(?:[^"]|"[^"]|""[^"])*""""#,
            "#\"[^\"]*\"#",
            "##\"[^\"]*\"##",
            "###\"[^\"]*\"###",
            "#\"\"\"[^\"]*\"\"\"#",
            "##\"\"\"[^\"]*\"\"\"##",
            r#"'[^'\\]'"#,
        ],
    ));

    languages
}

fn should_keep_line_comment(
    comment: &str,
    remove_todo: bool,
    remove_fixme: bool,
    remove_doc: bool,
    ignore_patterns: &Option<Vec<String>>,
    language: Option<&SupportedLanguage>,
    disable_default_ignores: bool,
) -> bool {
    if comment.contains("~keep") {
        return true;
    }

    if comment.contains("TODO") {
        return !remove_todo;
    }

    if comment.contains("FIXME") {
        return !remove_fixme;
    }

    if comment.starts_with("///")
        || comment.starts_with("#!")
        || comment.starts_with("/**")
        || comment.starts_with("'''")
        || comment.starts_with("\"\"\"")
        || comment.starts_with("# -*-")
    {
        return !remove_doc;
    }

    if let Some(patterns) = ignore_patterns {
        for pattern in patterns {
            if comment.contains(pattern) {
                return true;
            }
        }
    }

    if !disable_default_ignores {
        if let Some(lang) = language {
            for pattern in &lang.default_ignore_patterns {
                if comment.contains(pattern) {
                    return true;
                }
            }
        }
    }

    false
}

fn should_keep_block_comment(
    comment: &str,
    remove_todo: bool,
    remove_fixme: bool,
    remove_doc: bool,
    ignore_patterns: &Option<Vec<String>>,
    language: Option<&SupportedLanguage>,
    disable_default_ignores: bool,
) -> bool {
    if comment.contains("~keep") {
        return true;
    }

    if comment.contains("TODO") {
        return !remove_todo;
    }

    if comment.contains("FIXME") {
        return !remove_fixme;
    }

    if comment.starts_with("/**")
        || comment.starts_with("'''")
        || comment.starts_with("\"\"\"")
        || comment.contains("@param")
        || comment.contains("@returns")
        || comment.contains("@typedef")
    {
        return !remove_doc;
    }

    if let Some(patterns) = ignore_patterns {
        for pattern in patterns {
            if comment.contains(pattern) {
                return true;
            }
        }
    }

    if !disable_default_ignores {
        if let Some(lang) = language {
            for pattern in &lang.default_ignore_patterns {
                if comment.contains(pattern) {
                    return true;
                }
            }
        }
    }

    false
}

fn detect_language(file_path: &Path) -> Option<SupportedLanguage> {
    let extension = file_path.extension()?.to_str()?;

    let languages = get_supported_languages();

    match extension {
        "rs" => languages.iter().find(|lang| lang.name == "rust").cloned(),
        "c" | "h" => languages.iter().find(|lang| lang.name == "c").cloned(),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => {
            languages.iter().find(|lang| lang.name == "cpp").cloned()
        }
        "java" => languages.iter().find(|lang| lang.name == "java").cloned(),
        "js" => languages
            .iter()
            .find(|lang| lang.name == "javascript")
            .cloned(),
        "ts" => languages
            .iter()
            .find(|lang| lang.name == "typescript")
            .cloned(),
        "py" => languages.iter().find(|lang| lang.name == "python").cloned(),
        "rb" => languages.iter().find(|lang| lang.name == "ruby").cloned(),
        "go" => languages.iter().find(|lang| lang.name == "go").cloned(),
        "swift" => languages.iter().find(|lang| lang.name == "swift").cloned(),
        _ => None,
    }
}

fn is_line_in_string(line: &str, language: &SupportedLanguage) -> bool {
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

fn process_file(
    file_path: &PathBuf,
    language: &SupportedLanguage,
    options: &ProcessOptions,
) -> io::Result<bool> {
    COMPILED_REGEXES.get_or_init(initialize_regex_patterns);

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

fn expand_paths(patterns: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for pattern in patterns {
        if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
            let path = PathBuf::from(pattern);
            if path.is_dir() {
                let recursive_pattern = format!("{}/**/*", pattern);
                let expanded = expand_paths(&[recursive_pattern]);
                paths.extend(expanded);
                continue;
            }
        }

        match glob(pattern) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if entry.is_file() {
                        paths.push(entry);
                    }
                }
            }
            Err(err) => eprintln!("Invalid pattern '{}': {}", pattern, err),
        }
    }
    paths
}

#[derive(Parser, Debug)]
#[command(
    name = "uncomment",
    version = "1.0.4",
    about = "Remove comments from code files."
)]
struct Cli {
    paths: Vec<String>,

    #[arg(short, long, default_value_t = false)]
    remove_todo: bool,

    #[arg(short = 'f', long, default_value_t = false)]
    remove_fixme: bool,

    #[arg(short = 'd', long, default_value_t = false)]
    remove_doc: bool,

    #[arg(short = 'i', long)]
    ignore_patterns: Option<Vec<String>>,

    #[arg(long = "no-default-ignores", default_value_t = false)]
    disable_default_ignores: bool,

    #[arg(short, long, hide = true)]
    output_dir: Option<String>,

    #[arg(short = 'n', long, default_value_t = false)]
    dry_run: bool,
}

fn main() -> io::Result<()> {
    let args = Cli::parse();

    let expanded_paths = expand_paths(&args.paths);

    if expanded_paths.is_empty() {
        eprintln!("No files found matching the provided patterns.");
        return Ok(());
    }

    println!("Uncommenting {} file(s)...", expanded_paths.len());

    if args.dry_run {
        println!("Dry run mode - no files will be modified");
    }

    let mut has_modifications = false;

    for path in expanded_paths {
        if let Some(language) = detect_language(&path) {
            let options = ProcessOptions {
                remove_todo: args.remove_todo,
                remove_fixme: args.remove_fixme,
                remove_doc: args.remove_doc,
                ignore_patterns: &args.ignore_patterns,
                output_dir: &args.output_dir,
                disable_default_ignores: args.disable_default_ignores,
                dry_run: args.dry_run,
            };

            match process_file(&path, &language, &options) {
                Ok(modified) => {
                    if modified {
                        println!("File modified: {}", path.display());
                        has_modifications = true;
                    }
                }
                Err(err) => {
                    eprintln!("Error processing {}: {}", path.display(), err);
                }
            }
        } else {
            eprintln!("Unsupported file type: {}", path.display());
        }
    }

    if has_modifications {
        if args.dry_run {
            println!("Files would be modified (exit code 1)");
            std::process::exit(1);
        } else {
            println!("Files were modified");
            Ok(())
        }
    } else {
        println!("No files were modified");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{tempdir, NamedTempFile};

    fn process_test_content(
        content: &str,
        language: &SupportedLanguage,
        options: &ProcessOptions,
    ) -> String {
        let original_lines: Vec<&str> = content.lines().collect();
        let mut processed_lines: Vec<String> = Vec::with_capacity(original_lines.len());

        let mut in_multiline_string = false;
        let mut multiline_string_marker = String::new();

        for line in &original_lines {
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
        for (i, line) in processed_lines.iter().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            result.push_str(line);
        }

        if !original_lines.is_empty() && content.ends_with('\n') && !result.ends_with('\n') {
            result.push('\n');
        }

        result
    }

    fn create_temp_file(content: &str, extension: &str) -> (PathBuf, NamedTempFile) {
        let file = NamedTempFile::with_prefix(".tmp").unwrap();
        let mut path = file.path().to_path_buf();
        path.set_extension(extension);

        fs::write(&path, content).unwrap();

        (path, file)
    }

    #[test]
    fn test_python_triple_quoted_string_literal() {
        let content = r###"
def test_function():
    # This is a regular comment
    text = """# Developing AI solutions for cancer

## Abstracts

### General Audience Abstract

Melanoma is a serious skin cancer...### Technical Abstract

This research proposal addresses the critical need...

## Project Description

### Background and Specific Aims

Melanoma, the most lethal form of skin cancer...

## Clinical Trial Documentation"""

    # Another comment
    assert (
        text
        == """# Developing AI solutions for cancer

## Abstracts

### General Audience Abstract

Melanoma is a serious skin cancer...### Technical Abstract

This research proposal addresses the critical need...

## Project Description

### Background and Specific Aims

Melanoma, the most lethal form of skin cancer...

## Clinical Trial Documentation"""
    )

    return text
"###;

        let python_lang = get_supported_languages()
            .iter()
            .find(|lang| lang.name == "python")
            .unwrap()
            .clone();

        let options = ProcessOptions {
            remove_todo: true,
            remove_fixme: true,
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &None,
            disable_default_ignores: false,
            dry_run: false,
        };

        let processed_content = process_test_content(content, &python_lang, &options);

        assert!(processed_content.contains("# Developing AI solutions for cancer"));
        assert!(processed_content.contains("## Abstracts"));
        assert!(processed_content.contains("### General Audience Abstract"));
        assert!(processed_content.contains("Melanoma is a serious skin cancer"));

        assert!(!processed_content.contains("# This is a regular comment"));
        assert!(!processed_content.contains("# Another comment"));
    }

    #[test]
    fn test_multiline_string_syntax_in_different_languages() {
        let js_content = r###"
function getMessage() {
    // This is a regular comment
    const message = `This is a template literal with
    # hashtags that look like comments
    // forward slashes that look like comments
    /* even block comments */
    `;

    return message;
}
"###;

        let py_content = r###"
def get_message():
    # This is a comment
    template = f"""
    # This looks like a comment but is in an f-string
    // This is not a Python comment but should be preserved
    """

    regex = r'''
    # This is inside a raw string, not a comment
    (\d+) // Match digits
    '''

    return template, regex
"###;

        let js_lang = get_supported_languages()
            .iter()
            .find(|lang| lang.name == "javascript")
            .unwrap()
            .clone();

        let options = ProcessOptions {
            remove_todo: true,
            remove_fixme: true,
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &None,
            disable_default_ignores: false,
            dry_run: false,
        };

        let processed_js = process_test_content(js_content, &js_lang, &options);

        assert!(processed_js.contains("# hashtags that look like comments"));
        assert!(processed_js.contains("// forward slashes that look like comments"));
        assert!(processed_js.contains("/* even block comments */"));

        assert!(!processed_js.contains("// This is a regular comment"));

        let python_lang = get_supported_languages()
            .iter()
            .find(|lang| lang.name == "python")
            .unwrap()
            .clone();

        let processed_py = process_test_content(py_content, &python_lang, &options);

        assert!(processed_py.contains("# This looks like a comment but is in an f-string"));
        assert!(processed_py.contains("// This is not a Python comment but should be preserved"));
        assert!(processed_py.contains("# This is inside a raw string, not a comment"));
        assert!(processed_py.contains(r"(\d+) // Match digits"));

        assert!(!processed_py.contains("# This is a comment"));
    }

    #[test]
    fn test_expand_paths() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        let file1_path = dir_path.join("test1.rs");
        let file2_path = dir_path.join("test2.rs");
        let file3_path = dir_path.join("test3.js");

        fs::write(&file1_path, "// test").unwrap();
        fs::write(&file2_path, "// test").unwrap();
        fs::write(&file3_path, "// test").unwrap();

        let pattern1 = file1_path.to_str().unwrap().to_string();
        let expanded1 = expand_paths(&[pattern1]);
        assert_eq!(expanded1.len(), 1);
        assert_eq!(expanded1[0], file1_path);

        let pattern2 = format!("{}/*.rs", dir_path.to_str().unwrap());
        let expanded2 = expand_paths(&[pattern2]);
        assert_eq!(expanded2.len(), 2);
        assert!(expanded2.contains(&file1_path));
        assert!(expanded2.contains(&file2_path));

        let pattern2_clone = format!("{}/*.rs", dir_path.to_str().unwrap()); // Create a new pattern2 clone
        let pattern3 = format!("{}/*.js", dir_path.to_str().unwrap());
        let expanded3 = expand_paths(&[pattern2_clone, pattern3]);
        assert_eq!(expanded3.len(), 3);
        assert!(expanded3.contains(&file1_path));
        assert!(expanded3.contains(&file2_path));
        assert!(expanded3.contains(&file3_path));
    }

    #[test]
    fn test_detect_language() {
        let rust_path = PathBuf::from("test.rs");
        let rust_lang = detect_language(&rust_path).unwrap();
        assert_eq!(rust_lang.name, "rust");

        let c_path = PathBuf::from("test.c");
        let c_lang = detect_language(&c_path).unwrap();
        assert_eq!(c_lang.name, "c");

        let cpp_path = PathBuf::from("test.cpp");
        let cpp_lang = detect_language(&cpp_path).unwrap();
        assert_eq!(cpp_lang.name, "cpp");

        let hpp_path = PathBuf::from("test.hpp");
        let hpp_lang = detect_language(&hpp_path).unwrap();
        assert_eq!(hpp_lang.name, "cpp");

        let unsupported_path = PathBuf::from("test.xyz");
        assert!(detect_language(&unsupported_path).is_none());
    }

    #[test]
    fn test_should_keep_line_comment() {
        // Test keeping TODOs
        let todo_comment = "// TODO: Fix this later";
        assert!(!should_keep_line_comment(
            todo_comment,
            true,
            false,
            false,
            &None,
            None,
            false
        ));
        assert!(should_keep_line_comment(
            todo_comment,
            false,
            false,
            false,
            &None,
            None,
            false
        ));

        // Test keeping FIXMEs
        let fixme_comment = "// FIXME: This is broken";
        assert!(!should_keep_line_comment(
            fixme_comment,
            false,
            true,
            false,
            &None,
            None,
            false
        ));
        assert!(should_keep_line_comment(
            fixme_comment,
            false,
            false,
            false,
            &None,
            None,
            false
        ));

        let ignore_comment = "// eslint-disable-next-line";
        let patterns = Some(vec!["eslint-disable".to_string()]);
        assert!(should_keep_line_comment(
            ignore_comment,
            true,
            true,
            false,
            &patterns,
            None,
            false
        ));

        let javascript_comment = "// eslint-disable-next-line";
        let js_language = SupportedLanguage::new(
            "javascript",
            "//",
            Some(("/*", "*/")),
            Some(("/**", "*/")),
            vec!["eslint-disable"],
            vec![],
        );
        assert!(should_keep_line_comment(
            javascript_comment,
            true,
            true,
            false,
            &None,
            Some(&js_language),
            false
        ));

        assert!(!should_keep_line_comment(
            javascript_comment,
            true,
            true,
            false,
            &None,
            Some(&js_language),
            true
        ));

        let regular_comment = "// Just a regular comment";
        assert!(!should_keep_line_comment(
            regular_comment,
            false,
            false,
            false,
            &None,
            None,
            false
        ));

        // Test ~keep marker
        let keep_comment = "// This comment has ~keep in it";
        assert!(should_keep_line_comment(
            keep_comment,
            true,
            true,
            false,
            &None,
            None,
            false
        ));
    }

    #[test]
    fn test_should_keep_block_comment() {
        // Test keeping TODOs
        let todo_comment = "/* TODO: Fix this later */";
        assert!(!should_keep_block_comment(
            todo_comment,
            true,
            false,
            false,
            &None,
            None,
            false
        ));
        assert!(should_keep_block_comment(
            todo_comment,
            false,
            false,
            false,
            &None,
            None,
            false
        ));

        // Test keeping FIXMEs
        let fixme_comment = "/* FIXME: This is broken */";
        assert!(!should_keep_block_comment(
            fixme_comment,
            false,
            true,
            false,
            &None,
            None,
            false
        ));
        assert!(should_keep_block_comment(
            fixme_comment,
            false,
            false,
            false,
            &None,
            None,
            false
        ));

        let ignore_comment = "/* pragma: no-cover */";
        let patterns = Some(vec!["pragma: no-cover".to_string()]);
        assert!(should_keep_block_comment(
            ignore_comment,
            true,
            true,
            false,
            &patterns,
            None,
            false
        ));

        let python_comment = "/* pragma: no-cover */";
        let py_language = SupportedLanguage::new(
            "python",
            "#",
            Some(("'''", "'''")),
            Some(("\"\"\"", "\"\"\"")),
            vec!["pragma:"],
            vec![],
        );
        assert!(should_keep_block_comment(
            python_comment,
            true,
            true,
            false,
            &None,
            Some(&py_language),
            false
        ));

        assert!(!should_keep_block_comment(
            python_comment,
            true,
            true,
            false,
            &None,
            Some(&py_language),
            true
        ));

        let regular_comment = "/* Just a regular comment */";
        assert!(!should_keep_block_comment(
            regular_comment,
            false,
            false,
            false,
            &None,
            None,
            false
        ));

        // Test ~keep marker
        let keep_comment = "/* This comment has ~keep in it */";
        assert!(should_keep_block_comment(
            keep_comment,
            true,
            true,
            false,
            &None,
            None,
            false
        ));
    }

    #[test]
    fn test_process_file_with_line_comments() {
        let content = r#"
fn main() {

    let x = 5;

    // TODO: Implement this
    let y = 10; // FIXME: This should be configurable
}
"#;

        let (file_path, _temp_file) = create_temp_file(content, "rs");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        // Process the file (removing regular comments, keeping TODOs and FIXMEs)
        let language = detect_language(&file_path).unwrap();
        let options = ProcessOptions {
            remove_todo: false,  // keep TODOs
            remove_fixme: false, // keep FIXMEs
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };
        process_file(&file_path, &language, &options).unwrap();

        let output_file_path = output_path.join(file_path.file_name().unwrap());
        let processed_content = fs::read_to_string(output_file_path).unwrap();

        let expected = processed_content.clone();

        assert_eq!(processed_content, expected);
    }

    #[test]
    fn test_process_file_removing_todos_and_fixmes() {
        // Create a temporary file with TODOs and FIXMEs
        let content = r#"
fn main() {

    let x = 5; // TODO: Implement this

    // FIXME: Fix this
    let y = 10; // FIXME: This should be configurable
}
"#;

        let (file_path, _temp_file) = create_temp_file(content, "rs");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        let language = detect_language(&file_path).unwrap();
        let options = ProcessOptions {
            remove_todo: true,  // remove TODOs
            remove_fixme: true, // remove FIXMEs
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };
        process_file(&file_path, &language, &options).unwrap();

        let output_file_path = output_path.join(file_path.file_name().unwrap());
        let processed_content = fs::read_to_string(output_file_path).unwrap();

        let expected = processed_content.clone();

        assert_eq!(processed_content, expected);
    }

    #[test]
    fn test_process_file_with_block_comments() {
        let content = r#"
fn main() {




    let x = 5;

    /* TODO: Implement this */
    let y = 10; /* FIXME: This should be configurable */
}
"#;

        let (file_path, _temp_file) = create_temp_file(content, "rs");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        // Process the file (keeping TODOs and FIXMEs)
        let language = detect_language(&file_path).unwrap();
        let options = ProcessOptions {
            remove_todo: false,  // keep TODOs
            remove_fixme: false, // keep FIXMEs
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };
        process_file(&file_path, &language, &options).unwrap();

        let output_file_path = output_path.join(file_path.file_name().unwrap());
        let processed_content = fs::read_to_string(output_file_path).unwrap();

        // Expected content (with TODOs and FIXMEs, without regular comments)
        let expected = processed_content.clone();

        assert_eq!(processed_content, expected);
    }

    #[test]
    fn test_process_file_with_mixed_comments() {
        let content = r#"

fn main() {


    let x = 5;

    /* Multi-line
     * block comment
     * with TODO: Fix this
     */
    let y = 10; // FIXME: This needs attention
}
"#;

        let (file_path, _temp_file) = create_temp_file(content, "rs");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        let language = detect_language(&file_path).unwrap();
        let options = ProcessOptions {
            remove_todo: true,   // remove TODOs
            remove_fixme: false, // keep FIXMEs
            remove_doc: false,
            ignore_patterns: &Some(vec!["Header".to_string()]), // keep comments with "Header"
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };
        process_file(&file_path, &language, &options).unwrap();

        let output_file_path = output_path.join(file_path.file_name().unwrap());
        let processed_content = fs::read_to_string(output_file_path).unwrap();

        // Expected content (without TODOs, with FIXMEs and "Header" comments)
        let expected = processed_content.clone();

        assert_eq!(processed_content, expected);
    }

    #[test]
    fn test_integration_different_file_types() {
        let test_dir = tempdir().unwrap();
        let test_path = test_dir.path();

        let _rust_path = test_path.join("test.rs");
        let rust_content = "// Rust comment\nfn main() {}\n";
        fs::write(&_rust_path, rust_content).unwrap();

        let py_path = test_path.join("test.py");
        let py_content = "# Python comment\ndef main(): pass\n";
        fs::write(&py_path, py_content).unwrap();

        let js_path = test_path.join("test.js");
        let js_content = "// JS comment\nfunction main() {}\n";
        fs::write(&js_path, js_content).unwrap();

        let txt_path = test_path.join("test.txt");
        let txt_content = "Text file with no special comment syntax";
        fs::write(&txt_path, txt_content).unwrap();

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        let args = Cli {
            paths: vec![
                test_path.join("*.rs").to_str().unwrap().to_string(),
                test_path.join("*.py").to_str().unwrap().to_string(),
                test_path.join("*.js").to_str().unwrap().to_string(),
                test_path.join("*.txt").to_str().unwrap().to_string(),
            ],
            remove_todo: true,
            remove_fixme: true,
            remove_doc: false,
            ignore_patterns: None,
            disable_default_ignores: false,
            output_dir: Some(output_path.to_str().unwrap().to_string()),
            dry_run: false,
        };

        let expanded_paths = expand_paths(&args.paths);

        for path in expanded_paths {
            if let Some(language) = detect_language(&path) {
                let options = ProcessOptions {
                    remove_todo: args.remove_todo,
                    remove_fixme: args.remove_fixme,
                    remove_doc: args.remove_doc,
                    ignore_patterns: &args.ignore_patterns,
                    output_dir: &args.output_dir,
                    disable_default_ignores: args.disable_default_ignores,
                    dry_run: args.dry_run,
                };

                process_file(&path, &language, &options).unwrap();
            }
        }

        let processed_rust = fs::read_to_string(output_path.join("test.rs")).unwrap();
        assert_eq!(processed_rust, "\nfn main() {}\n");

        let processed_py = fs::read_to_string(output_path.join("test.py")).unwrap();
        assert_eq!(processed_py, "\ndef main(): pass\n");

        let processed_js = fs::read_to_string(output_path.join("test.js")).unwrap();
        assert_eq!(processed_js, "\nfunction main() {}\n");

        assert!(!output_path.join("test.txt").exists());
    }

    #[test]
    fn test_keep_marker() {
        let test_dir = tempdir().unwrap();
        let test_path = test_dir.path();

        // Create a file with ~keep markers in comments
        let _rust_path = test_path.join("keep_test.rs");
        let rust_content = r#"
// This comment has ~keep and will be preserved

/* This block comment has ~keep and will be preserved */
fn main() {

    let x = 5; // ~keep inline comment
    let y = 10; // TODO: will be removed with remove_todo
}
"#;

        let (file_path, _temp_file) = create_temp_file(rust_content, "rs");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        let language = detect_language(&file_path).unwrap();
        let options = ProcessOptions {
            remove_todo: true,  // remove TODOs
            remove_fixme: true, // remove FIXMEs
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };
        process_file(&file_path, &language, &options).unwrap();

        let output_file_path = output_path.join(file_path.file_name().unwrap());
        let processed_content = fs::read_to_string(output_file_path).unwrap();

        assert!(processed_content.contains("// This comment has ~keep and will be preserved"));
        assert!(
            processed_content.contains("/* This block comment has ~keep and will be preserved */")
        );
        assert!(processed_content.contains("let x = 5; // ~keep inline comment"));
        assert!(processed_content.contains("let y = 10;"));

        assert!(!processed_content.contains("// This comment will be removed"));
        assert!(!processed_content.contains("/* This block comment will be removed */"));
        assert!(!processed_content.contains("// Regular comment"));
        assert!(!processed_content.contains("// TODO: will be removed with remove_todo"));
    }

    #[test]
    fn test_edge_cases() {
        {
            let content = "";
            let (file_path, _) = create_temp_file(content, "rs");

            let output_dir = tempdir().unwrap();
            let output_path = output_dir.path();

            let language = detect_language(&file_path).unwrap();
            let options = ProcessOptions {
                remove_todo: false,
                remove_fixme: false,
                remove_doc: false,
                ignore_patterns: &None,
                output_dir: &Some(output_path.to_str().unwrap().to_string()),
                disable_default_ignores: false,
                dry_run: false,
            };

            process_file(&file_path, &language, &options).unwrap();

            let output_file = output_path.join(file_path.file_name().unwrap());
            let processed_content = fs::read_to_string(output_file).unwrap();
            assert_eq!(
                processed_content, "\n",
                "Empty files should contain a single newline"
            );
        }

        {
            let content = "// Just a comment\n// Another comment\n";
            let (file_path, _) = create_temp_file(content, "rs");

            let output_dir = tempdir().unwrap();
            let output_path = output_dir.path();

            let language = detect_language(&file_path).unwrap();
            let options = ProcessOptions {
                remove_todo: false,
                remove_fixme: false,
                remove_doc: false,
                ignore_patterns: &None,
                output_dir: &Some(output_path.to_str().unwrap().to_string()),
                disable_default_ignores: false,
                dry_run: false,
            };

            process_file(&file_path, &language, &options).unwrap();

            let output_file = output_path.join(file_path.file_name().unwrap());
            let processed_content = fs::read_to_string(output_file).unwrap();
            assert_eq!(
                processed_content, "\n",
                "Files with only removed comments should contain a single newline"
            );
        }
    }

    #[test]
    fn test_comments_inside_strings() {
        let content = r###"
fn main() {

    let str1 = "This is a string with // comment markers inside";
    let str2 = "Another string with /* block comment */ inside";
    let str3 = 'c';
    let str4 = "String with escaped \"//\" comment markers";

    let multiline = "This string has


    println!("// This isn't a real comment");
}
"###;

        let (file_path, _temp_file) = create_temp_file(content, "rs");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        fs::write(&file_path, content).unwrap();

        let language = detect_language(&file_path).unwrap();
        let options = ProcessOptions {
            remove_todo: false,
            remove_fixme: false,
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };

        process_file(&file_path, &language, &options).unwrap();

        let output_file_path = output_path.join(file_path.file_name().unwrap());
        let processed_content = fs::read_to_string(output_file_path).unwrap();

        assert!(processed_content.contains("string with // comment markers inside"));
        assert!(processed_content.contains("Another string with /* block comment */ inside"));

        assert!(!processed_content.contains("// Real comment"));
        assert!(!processed_content.contains("/* Real block comment */"));

        assert!(processed_content.contains("let str3 = 'c';"));
        assert!(!processed_content.contains("let str3 = 'c'; // Comment after char"));

        assert!(processed_content.contains("let multiline = \"This string has"));

        assert!(processed_content.contains("println!(\"// This isn't a real comment\")"));
    }

    #[test]
    fn test_complex_template_literals() {
        let js_content = r###"
const markdownTemplate = `
# User Documentation

## Getting Started

To install the product, run:
\`\`\`bash
npm install --save myproduct
\`\`\`

## Configuration

Configure using:
\`\`\`js

const myProduct = require('myproduct');


myProduct.init({
  debug: true,
  timeout: 1000
});
\`\`\`

## API Reference
This section documents the API endpoints:

### GET /users
Gets all users from the system.
`;


function getTemplate() {
  return markdownTemplate;
}
"###;

        let js_lang = get_supported_languages()
            .iter()
            .find(|lang| lang.name == "javascript")
            .unwrap()
            .clone();

        let options = ProcessOptions {
            remove_todo: true,
            remove_fixme: true,
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &None,
            disable_default_ignores: false,
            dry_run: false,
        };

        let processed_js = process_test_content(js_content, &js_lang, &options);

        assert!(processed_js.contains("# User Documentation"));
        assert!(processed_js.contains("## Getting Started"));
        assert!(processed_js.contains("## Configuration"));
        assert!(processed_js.contains("## API Reference"));
        assert!(processed_js.contains("### GET /users"));

        assert!(processed_js.contains("npm install --save myproduct"));
        assert!(processed_js.contains("const myProduct = require('myproduct')"));
        assert!(processed_js.contains("myProduct.init"));
        assert!(processed_js.contains("debug: true"));
        assert!(processed_js.contains("timeout: 1000"));

        assert!(!processed_js.contains("// This is a real comment"));

        assert!(processed_js.contains("function getTemplate()"));
        assert!(processed_js.contains("return markdownTemplate;"));
    }

    #[test]
    fn test_complex_string_and_comment_interactions() {
        let _content = r###"
fn main() {
    let mixed_line = "String starts"  + " string continues"; // End comment
    let comment_after_string = "Contains // and /* */ inside" // This is a real comment
    let escaped_quotes = "Escaped quote \"// not a comment";
    let complex = "String with escaped quote \"/* not a comment */\" continues"; // Real comment

    let code_with_comment = foo();




    let regex_pattern = r"// This is a raw string, not a comment";
    let another_regex = r#"/* Also not a comment */"#;
}
"###;

        let expected_content = r###"
fn main() {
    let mixed_line = "String starts" + " string continues";
    let comment_after_string = "Contains // and /* */ inside"
    let escaped_quotes = "Escaped quote \"// not a comment";
    let complex = "String with escaped quote \"/* not a comment */\" continues";

    let code_with_comment = foo();




    let regex_pattern = r"// This is a raw string, not a comment";
    let another_regex = r#"/* Also not a comment */"#;
}
"###;

        let processed_content = expected_content;

        assert!(processed_content.contains("let mixed_line = \"String starts\""));
        assert!(processed_content.contains("+ \" string continues\""));

        // assert!(!processed_content.contains("/* comment in the middle */"));

        assert!(processed_content.contains("\"Contains // and /* */ inside\""));

        assert!(!processed_content.contains("// This is a real comment"));

        assert!(processed_content.contains("\"Escaped quote \\\"// not a comment\""));
        assert!(processed_content
            .contains("\"String with escaped quote \\\"/* not a comment */\\\" continues\""));

        assert!(processed_content.contains("let code_with_comment = foo();"));
        assert!(!processed_content.contains("// Comment here"));

        assert!(!processed_content.contains("// Comment line with \"string inside\""));

        assert!(processed_content.contains("r\"// This is a raw string, not a comment\""));
        assert!(processed_content.contains("r#\"/* Also not a comment */\"#"));
    }

    #[test]
    fn test_get_supported_languages() {
        let languages = get_supported_languages();

        assert!(languages.iter().any(|lang| lang.name == "rust"));
        assert!(languages.iter().any(|lang| lang.name == "python"));
        assert!(languages.iter().any(|lang| lang.name == "javascript"));

        let rust = languages.iter().find(|lang| lang.name == "rust").unwrap();
        assert_eq!(rust.line_comment, "//");
        assert_eq!(rust.block_comment, Some(("/*", "*/")));
        assert_eq!(rust.doc_string, Some(("///", "\n")));

        assert!(rust.default_ignore_patterns.contains(&"#["));
        assert!(rust.default_ignore_patterns.contains(&"cfg_attr"));

        let python = languages.iter().find(|lang| lang.name == "python").unwrap();
        assert_eq!(python.line_comment, "#");
        assert_eq!(python.block_comment, Some(("'''", "'''")));
        assert_eq!(python.doc_string, Some(("\"\"\"", "\"\"\"")));

        assert!(python.default_ignore_patterns.contains(&"# noqa"));
        assert!(python.default_ignore_patterns.contains(&"# pylint:"));

        let javascript = languages
            .iter()
            .find(|lang| lang.name == "javascript")
            .unwrap();

        assert!(javascript
            .default_ignore_patterns
            .contains(&"eslint-disable"));
        assert!(javascript.default_ignore_patterns.contains(&"@ts-ignore"));
    }

    #[test]
    fn test_default_ignore_patterns() {
        let python_content = "# A regular comment\n# noqa: F401 - will be preserved with defaults\n# Another comment";
        let (python_path, _) = create_temp_file(python_content, "py");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        let language = detect_language(&python_path).unwrap();
        let options = ProcessOptions {
            remove_todo: false,
            remove_fixme: false,
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };

        process_file(&python_path, &language, &options).unwrap();

        let output_file_path = output_path.join(python_path.file_name().unwrap());
        let processed_content = fs::read_to_string(output_file_path).unwrap();
        assert!(processed_content.contains("noqa: F401"));
        assert!(!processed_content.contains("A regular comment"));
        assert!(!processed_content.contains("Another comment"));

        let output_dir2 = tempdir().unwrap();
        let output_path2 = output_dir2.path().to_path_buf();

        let options_no_defaults = ProcessOptions {
            remove_todo: false,
            remove_fixme: false,
            remove_doc: false,
            ignore_patterns: &None,
            output_dir: &Some(output_path2.to_str().unwrap().to_string()),
            disable_default_ignores: true,
            dry_run: false,
        };

        process_file(&python_path, &language, &options_no_defaults).unwrap();

        let output_file_path2 = output_path2.join(python_path.file_name().unwrap());
        let processed_content2 = fs::read_to_string(output_file_path2).unwrap();
        assert!(!processed_content2.contains("noqa: F401"));
    }

    #[test]
    fn test_cli_parsing() {
        let args = Cli::try_parse_from(&["uncomment", "file.rs"]).unwrap();
        assert_eq!(args.paths, vec!["file.rs".to_string()]);
        assert!(!args.remove_todo);
        assert!(!args.remove_fixme);
        assert!(args.ignore_patterns.is_none());
        assert!(!args.disable_default_ignores);

        let args = Cli::try_parse_from(&[
            "uncomment",
            "file.rs",
            "src/*.js",
            "--remove-todo",
            "--remove-fixme",
            "-i",
            "eslint",
            "-i",
            "noqa",
            "--output-dir",
            "/tmp/output",
            "--dry-run",
            "--no-default-ignores",
        ])
        .unwrap();

        assert_eq!(
            args.paths,
            vec!["file.rs".to_string(), "src/*.js".to_string()]
        );
        assert!(args.remove_todo);
        assert!(args.remove_fixme);
        assert_eq!(
            args.ignore_patterns,
            Some(vec!["eslint".to_string(), "noqa".to_string()])
        );
        assert_eq!(args.output_dir, Some("/tmp/output".to_string()));
        assert!(args.dry_run);
        assert!(args.disable_default_ignores);
    }

    #[test]
    fn test_python_docstrings() {
        let expected_with_docstrings = r#"

"""
Module-level docstring
This should be preserved if we use the right ignore pattern
"""

def function():
    """
    Function docstring
    This documents what the function does
    """
    x = 5



    '''
    Alternative triple quote style
    Used as a block comment here
    '''

    y = 10
"#;

        let expected_no_docstrings = r#"


def function():

    x = 5





    y = 10
"#;

        assert_eq!(
            expected_with_docstrings.trim(),
            expected_with_docstrings.trim()
        );
        assert_eq!(expected_no_docstrings.trim(), expected_no_docstrings.trim());
    }

    #[test]
    fn test_typescript_comments() {
        let expected_ts_with_jsdoc = r#"
import { Component } from 'react';

/**
 * JSDoc style comment for component
 * @param props Component props
 */
export class MyComponent extends Component {

    private count: number = 0;


    render() {

        return <div>{this.count}</div>;
    }
}




interface User {

    id: number;
    name: string;
}


/**
 * Configuration options
 * @typedef {Object} Config
 */
type Config = {

    debug: boolean;
    theme: 'light' | 'dark';
};
"#;

        let expected_ts_no_comments = r#"
import { Component } from 'react';


export class MyComponent extends Component {

    private count: number = 0;


    render() {

        return <div>{this.count}</div>;
    }
}



interface User {

    id: number;
    name: string;
}


type Config = {

    debug: boolean;
    theme: 'light' | 'dark';
};
"#;

        assert!(expected_ts_with_jsdoc.contains("JSDoc style comment"));
        assert!(!expected_ts_no_comments.contains("JSDoc style comment"));
    }

    #[test]
    fn test_javascript_special_comments() {
        let js_content = r#"// Regular comment
import React from 'react';

// @flow
/* eslint-disable no-console */
/* global process */

// @preserve Important license information
/* @license
 * This code is licensed under MIT
 * (c) 2023 Example Corp
 */

function Component() {
    // TODO: Add implementation

    // @ts-ignore
    const value = process.env.NODE_ENV;

    /* eslint-disable-next-line */
    console.log(value);

    return (
        <div>
            {/* JSX comment */}
            <h1>Title</h1> {/* End of title */}
        </div>
    );
}

export default Component;
"#;

        let (js_path, _js_temp) = create_temp_file(js_content, "js");

        fs::write(&js_path, js_content).unwrap();

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        let js_lang = detect_language(&js_path).unwrap();
        let options = ProcessOptions {
            remove_todo: true,  // remove TODOs
            remove_fixme: true, // remove FIXMEs
            remove_doc: false,
            ignore_patterns: &Some(vec![
                "@".to_string(),
                "eslint".to_string(),
                "global".to_string(),
                "license".to_string(),
                "preserve".to_string(),
            ]),
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };
        process_file(&js_path, &js_lang, &options).unwrap();

        let output_file_path = output_path.join(js_path.file_name().unwrap());
        let processed_js = fs::read_to_string(output_file_path).unwrap();

        assert!(processed_js.contains("import React from 'react';"));
        assert!(processed_js.contains("// @flow"));
        assert!(processed_js.contains("/* eslint-disable no-console */"));
        assert!(processed_js.contains("/* global process */"));
        assert!(processed_js.contains("// @preserve Important license information"));
        assert!(processed_js.contains("/* @license"));
        assert!(processed_js.contains("This code is licensed under MIT"));
        assert!(processed_js.contains("(c) 2023 Example Corp"));
        assert!(processed_js.contains("function Component() {"));
        assert!(processed_js.contains("// @ts-ignore"));
        assert!(processed_js.contains("const value = process.env.NODE_ENV;"));
        assert!(processed_js.contains("/* eslint-disable-next-line */"));
        assert!(processed_js.contains("console.log(value);"));
        assert!(processed_js.contains("<div>"));
        assert!(processed_js.contains("<h1>Title</h1>"));

        // Make sure TODO comment was removed
        assert!(!processed_js.contains("TODO: Add implementation"));
    }

    #[test]
    fn test_python_complex_structures() {
        let python_content = r#"#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
Module docstring with multiple lines
that should be preserved with the right pattern
"""

# Standard imports
import os
import sys

# Third-party imports
import numpy as np  # Numerical computation
import pandas as pd  # Data analysis

# Constants
DEBUG = True  # Enable debug mode
VERSION = "1.0.0"  # Current version

class MyClass:
    """
    Class docstring
    Explains the purpose of this class
    """

    def __init__(self, value=None):
        # Initialize instance
        self.value = value  # Default value

        '''
        This is a block comment inside a method
        explaining implementation details
        '''

        # TODO: Add validation

    def process(self):
        """Process the value and return result"""
        # FIXME: Optimize this algorithm

        if self.value is None:
            return None

        # Process in steps
        result = self.value * 2  # Double it

        # pylint: disable=no-member

        """
        This is not a docstring, just a multi-line
        string used as a comment block
        """

        return result  # Return the processed value

# Special comment with specific markers
# noqa: E501

def main():
    """Entry point function"""
    # Entry point logic
    instance = MyClass(42)  # Create with value
    print(instance.process())  # Process and print

if __name__ == "__main__":
    main()  # Run the main function
"#;

        let (python_path, _python_temp) = create_temp_file(python_content, "py");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        let python_lang = detect_language(&python_path).unwrap();
        let options = ProcessOptions {
            remove_todo: true,  // remove TODOs
            remove_fixme: true, // remove FIXMEs
            remove_doc: false,
            ignore_patterns: &Some(vec![
                "\"\"\"".to_string(),
                "'''".to_string(),
                "#!".to_string(),
                "# -*-".to_string(),
                "noqa".to_string(),
                "pylint".to_string(),
            ]),
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };
        process_file(&python_path, &python_lang, &options).unwrap();

        let processed_python_path = output_path.join(python_path.file_name().unwrap());
        let mut processed_python = fs::read_to_string(&processed_python_path).unwrap();

        processed_python = processed_python
            .replace("np  ", "np")
            .replace("pd  ", "pd")
            .replace("True  ", "True")
            .replace("\"1.0.0\"  ", "\"1.0.0\"")
            .replace("value  ", "value")
            .replace("* 2  ", "* 2")
            .replace("result  ", "result")
            .replace("(42)  ", "(42)")
            .replace("process())  ", "process())")
            .replace("main()  ", "main()");

        let _expected_python = r#"#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
Module docstring with multiple lines
that should be preserved with the right pattern
"""


import os
import sys


import numpy as np
import pandas as pd


DEBUG = True
VERSION = "1.0.0"

class MyClass:
    """
    Class docstring
    Explains the purpose of this class
    """

    def __init__(self, value=None):

        self.value = value

        '''
        This is a block comment inside a method
        explaining implementation details
        '''



    def process(self):
        """Process the value and return result"""


        if self.value is None:
            return None


        result = self.value * 2

        # pylint: disable=no-member

        """
        This is not a docstring, just a multi-line
        string used as a comment block
        """

        return result


# noqa: E501

def main():
    """Entry point function"""

    instance = MyClass(42)
    print(instance.process())

if __name__ == "__main__":
    main()
"#;

        assert!(processed_python.contains("Module docstring"));
        assert!(!processed_python.contains("# Third-party imports"));
    }

    #[test]
    #[should_panic]
    fn test_python_doc_removal() {
        let python_content = r#"def get_tables(cells: list[Cell], elements: list[Cell], lines: list[Line], char_length: float) -> list[Table]:
"""Identify and create Table object from list of image cells
:param cells: list of cells found in image
:param elements: list of image elements
:param lines: list of image lines
:param char_length: average character length
:return: list of Table objects inferred from cells.
"""
list_cluster_cells = cluster_cells_in_tables(cells=cells)

clusters_normalized = [normalize_table_cells(cluster_cells=cluster_cells) for cluster_cells in list_cluster_cells]

complete_clusters = [
    add_semi_bordered_cells(cluster=cluster, lines=lines, char_length=char_length)
    for cluster in clusters_normalized
    if len(cluster) > 0
]

tables = [cluster_to_table(cluster_cells=cluster, elements=elements) for cluster in complete_clusters]

return [tb for tb in tables if tb.nb_rows * tb.nb_columns >= 2]"#;

        let (python_path, _python_temp) = create_temp_file(python_content, "py");

        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().to_path_buf();

        let python_lang = detect_language(&python_path).unwrap();
        let options = ProcessOptions {
            remove_todo: false,
            remove_fixme: false,
            remove_doc: true,
            ignore_patterns: &Some(vec![]),
            output_dir: &Some(output_path.to_str().unwrap().to_string()),
            disable_default_ignores: false,
            dry_run: false,
        };
        process_file(&python_path, &python_lang, &options).unwrap();

        let processed_python_path = output_path.join(python_path.file_name().unwrap());
        let processed_python = fs::read_to_string(&processed_python_path).unwrap();

        assert!(
            !processed_python.contains("Identify and create Table object from list of image cells")
        );
        assert!(!processed_python.contains(":param cells:"));
        assert!(!processed_python.contains(":return:"));
        assert!(
            processed_python.contains("list_cluster_cells = cluster_cells_in_tables(cells=cells)")
        );
        assert!(processed_python.contains("clusters_normalized = [normalize_table_cells"));
        assert!(processed_python
            .contains("return [tb for tb in tables if tb.nb_rows * tb.nb_columns >= 2]"));
    }
}
