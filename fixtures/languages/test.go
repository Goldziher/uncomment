package main

//go:build linux
// +build linux

//go:embed hello.txt
var embedded string

import "C"

// this comment should be removed
func main() {}
