#![feature(test)]
extern crate test;

use std::fs;
use std::path::Path;
use tempfile::tempdir;
use test::Bencher;
use uncomment::languages::registry::LanguageRegistry;
use uncomment::processor::{ProcessingOptions, Processor};

fn create_test_file(content: &str, extension: &str) -> tempfile::TempPath {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join(format!("test.{}", extension));
    fs::write(&file_path, content).unwrap();
    file_path.into_temp_path()
}

#[bench]
fn bench_small_python_file(b: &mut Bencher) {
    let content = r#"
# This is a comment
def hello():
    # Another comment
    print("Hello")  # Inline comment
    """
    This is a docstring
    """
    return True

# TODO: Add more functions
# FIXME: Handle edge cases
"#;

    let file = create_test_file(content, "py");
    let mut processor = Processor::new();
    let options = ProcessingOptions {
        remove_todo: false,
        remove_fixme: false,
        remove_doc: false,
        custom_preserve_patterns: vec![],
        use_default_ignores: true,
        dry_run: true,
        respect_gitignore: false,
        traverse_git_repos: false,
    };

    b.iter(|| processor.process_file(file.as_ref(), &options).unwrap());
}

#[bench]
fn bench_large_python_file(b: &mut Bencher) {
    // Generate a large Python file with many comments
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!(
            r#"
# Function {i} comment
def function_{i}():
    # Implementation comment
    x = {i}  # Inline comment
    # Multi-line
    # comment block
    # with several lines
    """
    Docstring for function {i}
    With multiple lines
    """
    return x * 2
"#,
            i = i
        ));
    }

    let file = create_test_file(&content, "py");
    let mut processor = Processor::new();
    let options = ProcessingOptions {
        remove_todo: false,
        remove_fixme: false,
        remove_doc: false,
        custom_preserve_patterns: vec![],
        use_default_ignores: true,
        dry_run: true,
        respect_gitignore: false,
        traverse_git_repos: false,
    };

    b.iter(|| processor.process_file(file.as_ref(), &options).unwrap());
}

#[bench]
fn bench_javascript_file(b: &mut Bencher) {
    let content = r#"
// Main application file
import React from 'react';

/* Multi-line comment
   explaining the component */
function App() {
    // Component logic
    const [state, setState] = useState(0); // State hook

    /* Event handler
       with detailed explanation */
    const handleClick = () => {
        // Update state
        setState(state + 1);
    };

    return <div>{state}</div>;
}

// TODO: Add PropTypes
// eslint-disable-next-line
export default App;
"#;

    let file = create_test_file(content, "js");
    let mut processor = Processor::new();
    let options = ProcessingOptions {
        remove_todo: false,
        remove_fixme: false,
        remove_doc: false,
        custom_preserve_patterns: vec![],
        use_default_ignores: true,
        dry_run: true,
        respect_gitignore: false,
        traverse_git_repos: false,
    };

    b.iter(|| processor.process_file(file.as_ref(), &options).unwrap());
}

#[bench]
fn bench_parser_initialization(b: &mut Bencher) {
    let registry = LanguageRegistry::new();
    let python_config = registry.get_language("python").unwrap();

    b.iter(|| {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(python_config.tree_sitter_language())
            .unwrap();
        parser
    });
}

#[bench]
fn bench_comment_removal_algorithm(b: &mut Bencher) {
    use uncomment::ast::visitor::{CommentInfo, CommentVisitor};
    use uncomment::rules::preservation::PreservationRule;

    let content = r#"
# Comment 1
code_line_1()
# Comment 2
code_line_2()  # Inline comment
# Comment 3
code_line_3()
"#;

    let comments = vec![
        CommentInfo {
            start_byte: 0,
            end_byte: 11,
            start_row: 0,
            end_row: 0,
            content: "# Comment 1".to_string(),
            node_type: "comment".to_string(),
            should_preserve: false,
        },
        CommentInfo {
            start_byte: 26,
            end_byte: 37,
            start_row: 2,
            end_row: 2,
            content: "# Comment 2".to_string(),
            node_type: "comment".to_string(),
            should_preserve: false,
        },
        CommentInfo {
            start_byte: 54,
            end_byte: 70,
            start_row: 3,
            end_row: 3,
            content: "# Inline comment".to_string(),
            node_type: "comment".to_string(),
            should_preserve: false,
        },
    ];

    let processor = Processor::new();

    b.iter(|| {
        // This would test the comment removal algorithm
        // In practice, we'd need to expose this method or test indirectly
        comments.len()
    });
}
