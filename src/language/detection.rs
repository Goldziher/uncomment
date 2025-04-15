use crate::language::definitions::get_supported_languages;
use crate::models::language::SupportedLanguage;
use regex::Regex;
use std::path::Path;

/// Detect the programming language of a file based on its extension
pub fn detect_language(file_path: &Path) -> Option<SupportedLanguage> {
    let file_name = file_path.file_name()?.to_str()?;

    let languages = get_supported_languages();

    // Use regex to match file extensions
    for language in languages.iter() {
        // Compile the regex pattern
        if let Ok(pattern) = Regex::new(language.extension_regex) {
            if pattern.is_match(file_name) {
                return Some(language.clone());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_language() {
        // Standard file extensions
        let rust_path = PathBuf::from("test.rs");
        let rust_lang = detect_language(&rust_path).unwrap();
        assert_eq!(rust_lang.name, "rust");

        let c_path = PathBuf::from("test.c");
        let c_lang = detect_language(&c_path).unwrap();
        assert_eq!(c_lang.name, "c");

        let h_path = PathBuf::from("test.h");
        let h_lang = detect_language(&h_path).unwrap();
        assert_eq!(h_lang.name, "c");

        let cpp_path = PathBuf::from("test.cpp");
        let cpp_lang = detect_language(&cpp_path).unwrap();
        assert_eq!(cpp_lang.name, "cpp");

        let hpp_path = PathBuf::from("test.hpp");
        let hpp_lang = detect_language(&hpp_path).unwrap();
        assert_eq!(hpp_lang.name, "cpp");

        // JavaScript and variants
        let js_path = PathBuf::from("test.js");
        let js_lang = detect_language(&js_path).unwrap();
        assert_eq!(js_lang.name, "javascript");

        let jsx_path = PathBuf::from("test.jsx");
        let jsx_lang = detect_language(&jsx_path).unwrap();
        assert_eq!(jsx_lang.name, "javascript");

        let mjs_path = PathBuf::from("test.mjs");
        let mjs_lang = detect_language(&mjs_path).unwrap();
        assert_eq!(mjs_lang.name, "javascript");

        let cjs_path = PathBuf::from("test.cjs");
        let cjs_lang = detect_language(&cjs_path).unwrap();
        assert_eq!(cjs_lang.name, "javascript");

        // TypeScript and variants
        let ts_path = PathBuf::from("test.ts");
        let ts_lang = detect_language(&ts_path).unwrap();
        assert_eq!(ts_lang.name, "typescript");

        let tsx_path = PathBuf::from("test.tsx");
        let tsx_lang = detect_language(&tsx_path).unwrap();
        assert_eq!(tsx_lang.name, "typescript");

        let mts_path = PathBuf::from("test.mts");
        let mts_lang = detect_language(&mts_path).unwrap();
        assert_eq!(mts_lang.name, "typescript");

        let cts_path = PathBuf::from("test.cts");
        let cts_lang = detect_language(&cts_path).unwrap();
        assert_eq!(cts_lang.name, "typescript");

        // Declaration files
        let dts_path = PathBuf::from("test.d.ts");
        let dts_lang = detect_language(&dts_path).unwrap();
        assert_eq!(dts_lang.name, "typescript");

        let d_mts_path = PathBuf::from("test.d.mts");
        let d_mts_lang = detect_language(&d_mts_path).unwrap();
        assert_eq!(d_mts_lang.name, "typescript");

        let d_cts_path = PathBuf::from("test.d.cts");
        let d_cts_lang = detect_language(&d_cts_path).unwrap();
        assert_eq!(d_cts_lang.name, "typescript");

        // Other languages
        let py_path = PathBuf::from("test.py");
        let py_lang = detect_language(&py_path).unwrap();
        assert_eq!(py_lang.name, "python");

        let rb_path = PathBuf::from("test.rb");
        let rb_lang = detect_language(&rb_path).unwrap();
        assert_eq!(rb_lang.name, "ruby");

        let go_path = PathBuf::from("test.go");
        let go_lang = detect_language(&go_path).unwrap();
        assert_eq!(go_lang.name, "go");

        let swift_path = PathBuf::from("test.swift");
        let swift_lang = detect_language(&swift_path).unwrap();
        assert_eq!(swift_lang.name, "swift");

        let unsupported_path = PathBuf::from("test.xyz");
        assert!(detect_language(&unsupported_path).is_none());
    }
}
