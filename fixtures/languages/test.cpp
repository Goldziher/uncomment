#include <iostream>
#include <string>

// This line comment should be removed

/* This block comment should be removed */

int main() {
    std::string url = "http://example.com//path";
    std::string msg = "Hello // world";
    int x = 42; // NOLINT
    // TODO: add error handling
    std::cout << msg << std::endl;
    return 0;
}
