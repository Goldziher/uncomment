#!/usr/bin/perl
# This comment should be removed
use strict;
use warnings;

# TODO: add input validation
# noqa: complex-regex
my $name = "world";
my $msg = "This # is not a comment";
my $url = "http://example.com#anchor";

# This comment should be removed
sub greet {
    my ($who) = @_;
    print "Hello, $who!\n";
}

greet($name);
