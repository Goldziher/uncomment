package main

//go:build linux
// +build linux

//go:embed hello.txt
var embedded string

// #cgo CFLAGS: -I.
// #include <stdlib.h>
import "C"

// this comment should be removed
func main() {}
