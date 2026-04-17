// This line comment should be removed
/* This block comment should be removed */

class Greeter {
    // TODO: add logging
    String name

    /* @suppress("GroovyUnusedDeclaration") */
    String greet() {
        def url = "http://example.com // not a comment"
        def path = "C:\\path\\to\\file"
        // This comment should be removed
        return "Hello, ${name}!"
    }
}

def g = new Greeter(name: "world")
println g.greet()
