#include <stdio.h>

#pragma once

// This line comment should be removed

/* This block comment should be removed */

int main() {
    char *url = "http://example.com//path";
    char *msg = "Hello // world";
    // TODO: check return value
    printf("%s\n", msg);
    return 0;
}
