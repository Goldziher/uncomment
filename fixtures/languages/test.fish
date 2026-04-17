#!/usr/bin/env fish
# This comment should be removed

set greeting "Hello"
# TODO: support multiple locales
set msg "This # is not a comment"
set url "http://example.com#anchor"

# FIXME: handle empty arguments
function greet
    echo $greeting $argv
end

greet "world"
