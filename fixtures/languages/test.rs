// This line comment should be removed

/// This is a doc comment that should be preserved
#[allow(dead_code)]
fn greet(name: &str) -> String {
    /* This block comment should be removed */
    let url = "http://example.com//path";
    let msg = "Hello // world";
    // TODO: add error handling
    format!("{}, {}!", msg, name)
}

fn main() {
    let result = greet("Alice");
    println!("{}", result);
}
