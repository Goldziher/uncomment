use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub struct SupportedLanguage {
    pub name: &'static str,
    pub line_comment: &'static str,
    pub block_comment: Option<(&'static str, &'static str)>,
    pub doc_string: Option<(&'static str, &'static str)>,
    pub default_ignore_patterns: Vec<&'static str>,
    pub string_regex_patterns: Vec<&'static str>,
    pub extension_regex: &'static str,
}

impl SupportedLanguage {
    pub fn new(
        name: &'static str,
        line_comment: &'static str,
        block_comment: Option<(&'static str, &'static str)>,
        doc_string: Option<(&'static str, &'static str)>,
        default_ignore_patterns: Vec<&'static str>,
        string_regex_patterns: Vec<&'static str>,
        extension_regex: &'static str,
    ) -> Self {
        Self {
            name,
            line_comment,
            block_comment,
            doc_string,
            default_ignore_patterns,
            string_regex_patterns,
            extension_regex,
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
            extension_regex: self.extension_regex,
        }
    }
}

impl PartialEq for SupportedLanguage {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for SupportedLanguage {}

impl Hash for SupportedLanguage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
