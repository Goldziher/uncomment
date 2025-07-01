use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Test Python docstring detection specifically
#[test]
fn test_python_docstring_detection() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create config that removes docstrings but preserves other strings
    let config_content = r#"
[global]
remove_todos = false
remove_docs = true

[languages.python]
name = "Python"
extensions = [".py"]
comment_nodes = ["comment"]
doc_comment_nodes = ["string"]
remove_docs = true
"#;

    fs::write(root.join(".uncommentrc.toml"), config_content).unwrap();

    // Create Python file with various string types
    let py_file = root.join("test_docstrings.py");
    let py_content = r#""""Module level docstring"""

def function_with_docstring():
    """Function docstring"""
    regular_string = "This is not a docstring"
    return regular_string

class TestClass:
    """Class docstring"""

    def method(self):
        """Method docstring"""
        another_string = "Also not a docstring"
        return another_string

    def method_no_docstring(self):
        x = "assignment first"
        """Not a docstring because assignment comes first"""
        return "string"

# Regular comment
def func_no_docstring():
    x = "Just a string"
    return x
"#;
    fs::write(&py_file, py_content).unwrap();

    let uncomment_path = get_binary_path();

    // Process the file
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["test_docstrings.py"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check the result
    let result_content = fs::read_to_string(&py_file).unwrap();

    // Docstrings should be removed
    assert!(!result_content.contains("Module level docstring"));
    assert!(!result_content.contains("Function docstring"));
    assert!(!result_content.contains("Class docstring"));
    assert!(!result_content.contains("Method docstring"));

    // Regular strings should be preserved
    assert!(result_content.contains("This is not a docstring"));
    assert!(result_content.contains("Also not a docstring"));
    assert!(result_content.contains("Just a string"));
    assert!(result_content.contains("string")); // return value

    // Non-docstring triple-quoted string should be preserved
    assert!(result_content.contains("Not a docstring because assignment comes first"));

    // Regular comments should be removed (comment_nodes = ["comment"])
    assert!(!result_content.contains("# Regular comment"));
}

#[test]
fn test_python_docstring_vs_regular_strings() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Config that removes docstrings
    let config_content = r#"
[global]
remove_docs = true

[languages.python]
name = "Python"
extensions = [".py"]
comment_nodes = ["comment"]
doc_comment_nodes = ["string"]
remove_docs = true
"#;

    fs::write(root.join(".uncommentrc.toml"), config_content).unwrap();

    // Python file with edge cases
    let py_file = root.join("test_edge_cases.py");
    let py_content = r#""""This is a module docstring"""

def func1():
    """This is a function docstring"""
    x = """This is NOT a docstring - it's an assignment"""
    y = "Regular string"
    return x + y

def func2():
    x = "assignment comes first"
    """This is NOT a docstring because assignment came first"""
    return "result"

class MyClass:
    """This is a class docstring"""

    def __init__(self):
        """This is a method docstring"""
        self.value = "This is not a docstring"

    def method(self):
        var = "First statement is assignment"
        """So this is NOT a docstring"""
        return var
"#;
    fs::write(&py_file, py_content).unwrap();

    let uncomment_path = get_binary_path();

    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["test_edge_cases.py"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let result_content = fs::read_to_string(&py_file).unwrap();

    // These should be removed (actual docstrings)
    assert!(!result_content.contains("This is a module docstring"));
    assert!(!result_content.contains("This is a function docstring"));
    assert!(!result_content.contains("This is a class docstring"));
    assert!(!result_content.contains("This is a method docstring"));

    // These should be preserved (not docstrings)
    assert!(result_content.contains("This is NOT a docstring - it's an assignment"));
    assert!(result_content.contains("Regular string"));
    assert!(result_content.contains("This is NOT a docstring because assignment came first"));
    assert!(result_content.contains("This is not a docstring"));
    assert!(result_content.contains("First statement is assignment"));
    assert!(result_content.contains("So this is NOT a docstring"));
    assert!(result_content.contains("result"));
    assert!(result_content.contains("assignment comes first"));
}

// Helper function to get the binary path
fn get_binary_path() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("uncomment")
}
