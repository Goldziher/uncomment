use crate::models::language::SupportedLanguage;

/// Determines whether a line comment should be kept or removed
pub fn should_keep_line_comment(
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

/// Determines whether a block comment should be kept or removed
pub fn should_keep_block_comment(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::language::SupportedLanguage;

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
            r"\.(?:js|mjs|cjs|jsx)$",
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
            r"\.(?:py|pyw|pyi)$",
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
}
