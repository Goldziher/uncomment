use crate::models::language::SupportedLanguage;
use std::collections::HashSet;

/// Returns a HashSet of all supported programming languages
pub fn get_supported_languages() -> HashSet<SupportedLanguage> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
