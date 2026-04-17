const std = @import("std");

/// Doc comment for the function
fn add(a: i32, b: i32) i32 {
    // This comment should be removed
    return a + b;
}

pub fn main() void {
    const greeting = "Hello // not a comment";
    // TODO: add error handling
    _ = add(1, 2);
}
