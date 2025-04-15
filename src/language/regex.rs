use crate::language::definitions::get_supported_languages;
use crate::models::language::SupportedLanguage;
use regex::{Regex, RegexSet};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Compiled regular expressions for a language
#[derive(Clone)]
pub struct CompiledRegexes {
    pub regex_set: RegexSet,
    pub regex_patterns: Vec<Regex>,
    pub string_spans: HashMap<String, Vec<(usize, usize)>>,
}

/// Static cache of compiled regexes for all languages
static COMPILED_REGEXES: OnceLock<HashMap<&'static str, CompiledRegexes>> = OnceLock::new();

/// Get or initialize the compiled regexes for a language
pub fn get_or_compile_regexes(language: &SupportedLanguage) -> &CompiledRegexes {
    let regexes_map = COMPILED_REGEXES.get_or_init(initialize_regex_patterns);

    regexes_map.get(language.name).unwrap_or_else(|| {
        panic!("Language {} not initialized in regex cache", language.name);
    })
}

/// Initialize regex patterns for all supported languages
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

/// Find string spans in a line of code
pub fn find_string_spans(line: &str, language: &SupportedLanguage) -> Vec<(usize, usize)> {
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

/// Check if a position in a line is inside a string
pub fn is_in_string(line: &str, pos: usize, language: &SupportedLanguage) -> bool {
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
