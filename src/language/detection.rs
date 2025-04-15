use crate::language::definitions::get_supported_languages;
use crate::models::language::SupportedLanguage;
use std::path::Path;

/// Detect the programming language of a file based on its extension
pub fn detect_language(file_path: &Path) -> Option<SupportedLanguage> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
